use std::collections::VecDeque;

use rust_decimal::Decimal;

use crate::content::cards::{CardKeyword, CardPlayCtx, CardType, TargetType};
use crate::core::command::CommandError;
use crate::core::effect::{
    CardFilter, DamageOp, DamageResult, Effect, MoveReason, Source, UpgradeMode,
};
use crate::core::engine::{combat_result_for_state, StepResult};
use crate::core::event::{
    BlockGained, CardDrawn, CardExhausted, CardUpgraded, CardsShuffled, CreatureHpChanged, Event,
    PowerApplied, ResourceChanged,
};
use crate::core::ids::{ChoiceId, CreatureId};
use crate::core::listener::ListenerRef;
use crate::core::log::{LogEntry, StateChange};
use crate::core::query::{BlockCalc, DamageCalc, Decision, PreventReason, ResourceCostCalc};
use crate::core::rules::{RuleCtx, RulePipeline};
use crate::core::state::{
    decimal_to_i32_trunc, CardCost, CombatPhase, GameState, PileKind, ResourceKind, Side,
    StateError, BASE_HAND_DRAW_COUNT,
};
use crate::registry::StaticRegistry;

#[derive(Default)]
pub struct EffectResolver {
    queue: VecDeque<Effect>,
    pending_choice: Option<crate::core::effect::ChoiceRequest>,
    log: Vec<LogEntry>,
    last_card_payment: std::collections::BTreeMap<crate::core::ids::CardInstanceId, CardPayment>,
}

impl EffectResolver {
    pub fn enqueue(&mut self, effect: Effect) {
        self.queue.push_back(effect);
    }

    pub fn enqueue_all(&mut self, effects: impl IntoIterator<Item = Effect>) {
        self.queue.extend(effects);
    }

    pub fn has_pending_choice(&self) -> bool {
        self.pending_choice.is_some()
    }

    pub fn submit_choice(&mut self, choice: ChoiceId) -> Result<(), CommandError> {
        let pending = self
            .pending_choice
            .take()
            .ok_or(CommandError::UnexpectedChoice)?;
        if pending.id != choice {
            self.pending_choice = Some(pending.clone());
            return Err(CommandError::ChoiceMismatch {
                expected: pending.id,
                actual: choice,
            });
        }
        self.enqueue(Effect::ResolveChoice(choice));
        Ok(())
    }

    pub fn take_log(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.log)
    }

    fn clear_aborted_resolution(&mut self) {
        self.queue.clear();
        self.last_card_payment.clear();
    }

    pub fn drain(&mut self, state: &mut GameState, registry: &StaticRegistry) -> StepResult {
        while let Some(effect) = self.queue.pop_front() {
            self.log.push(LogEntry::EffectStarted(effect.clone()));

            match self.apply_effect(state, registry, effect) {
                ApplyResult::Continue(more) => self.queue.extend(more),
                ApplyResult::NeedChoice(choice) => {
                    self.pending_choice = Some(choice.clone());
                    self.log.push(LogEntry::ChoiceRequested(choice.clone()));
                    return StepResult::NeedChoice(choice, self.take_log());
                }
                ApplyResult::CombatOver(result) => {
                    self.clear_aborted_resolution();
                    self.log.push(LogEntry::CombatEnded(result.clone()));
                    return StepResult::CombatOver(result, self.take_log());
                }
                ApplyResult::Rejected(error) => {
                    self.clear_aborted_resolution();
                    return StepResult::Rejected(error, self.take_log());
                }
                ApplyResult::StateError(error) => {
                    self.clear_aborted_resolution();
                    self.log.push(LogEntry::Error(error.clone()));
                    return StepResult::Failed(error, self.take_log());
                }
            }
        }

        StepResult::Done(self.take_log())
    }

    fn apply_effect(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        effect: Effect,
    ) -> ApplyResult {
        match effect {
            Effect::Trigger(event) => {
                self.record_event_stats(state, registry, &event);
                self.log.push(LogEntry::EventTriggered(event.clone()));
                ApplyResult::Continue(RulePipeline::notify(registry, state, &event))
            }
            Effect::ValidateCardPlay {
                player,
                card,
                target,
            } => {
                if !state.card_is_in_pile(card, PileKind::Hand) {
                    return ApplyResult::Rejected(CommandError::InvalidCard(card));
                }
                if let Err(reason) = validate_card_target(state, registry, card, target) {
                    return ApplyResult::Rejected(CommandError::Prevented(reason));
                }

                let decision = RulePipeline::should_play(registry, state, card, target);
                self.log.push(LogEntry::DecisionMade(decision.clone()));
                match decision {
                    Decision::Allow => {
                        match self.resolve_card_payment(state, registry, player, card, false) {
                            Ok(_) => ApplyResult::Continue(Vec::new()),
                            Err(error) => ApplyResult::Rejected(error),
                        }
                    }
                    Decision::Prevent { reason, .. } => {
                        ApplyResult::Rejected(CommandError::Prevented(reason))
                    }
                }
            }
            Effect::SpendResource {
                player,
                resource,
                amount,
            } => {
                if amount == 0 {
                    return ApplyResult::Continue(Vec::new());
                }
                match state.spend_resource(player, resource, amount) {
                    Ok(()) => {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::ResourceSpent {
                                player,
                                resource,
                                amount,
                            }));
                        ApplyResult::Continue(vec![Effect::Trigger(Event::ResourceSpent(
                            ResourceChanged {
                                player,
                                resource,
                                amount,
                            },
                        ))])
                    }
                    Err(StateError::NotEnoughResource { resource, .. }) => ApplyResult::Rejected(
                        CommandError::Prevented(PreventReason::InsufficientResource(resource)),
                    ),
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::GainResource {
                player,
                resource,
                amount,
            } => {
                if amount == 0 {
                    return ApplyResult::Continue(Vec::new());
                }
                match state.gain_resource(player, resource, amount) {
                    Ok(()) => {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::ResourceGained {
                                player,
                                resource,
                                amount,
                            }));
                        ApplyResult::Continue(vec![Effect::Trigger(Event::ResourceGained(
                            ResourceChanged {
                                player,
                                resource,
                                amount,
                            },
                        ))])
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::PayCardCosts { player, card } => {
                match self.resolve_card_payment(state, registry, player, card, true) {
                    Ok(payment) => {
                        self.last_card_payment.insert(card, payment);
                        self.spend_card_payment(state, player, payment)
                    }
                    Err(error) => ApplyResult::Rejected(error),
                }
            }
            Effect::ExecuteCardBody { card, target, .. } => {
                let payment = self.last_card_payment.remove(&card).unwrap_or_default();
                let effects = state
                    .card(card)
                    .and_then(|card_state| registry.cards.get(card_state.def))
                    .map(|def| {
                        let ctx = CardPlayCtx {
                            state,
                            registry,
                            paid_energy: payment.energy,
                            paid_stars: payment.stars,
                        };
                        (def.play)(&ctx, card, target)
                    })
                    .unwrap_or_default();
                self.apply_immediate_effects(state, registry, effects)
            }
            Effect::DealDamage(op) => self.apply_damage(state, registry, op),
            Effect::DealDamageToAllEnemies(op) => {
                let mut effects = Vec::new();
                for _ in 0..op.hit_count {
                    for target in state.alive_monster_ids() {
                        effects.push(Effect::DealDamage(DamageOp {
                            source: op.source,
                            dealer: op.dealer,
                            target,
                            base_amount: op.base_amount,
                            kind: op.kind,
                            flags: op.flags,
                        }));
                    }
                }
                ApplyResult::Continue(effects)
            }
            Effect::DealDamageToRandomEnemy(op) => {
                let mut effects = Vec::new();
                for _ in 0..op.hit_count {
                    let enemies = state.alive_monster_ids();
                    if enemies.is_empty() {
                        break;
                    }
                    let Some(index) = state.rng.combat_targets.next_usize(enemies.len()) else {
                        break;
                    };
                    effects.push(Effect::DealDamage(DamageOp {
                        source: op.source,
                        dealer: op.dealer,
                        target: enemies[index],
                        base_amount: op.base_amount,
                        kind: op.kind,
                        flags: op.flags,
                    }));
                }
                ApplyResult::Continue(effects)
            }
            Effect::LoseHp {
                target,
                amount,
                source,
            } => match state.lose_hp(target, amount) {
                Ok(actual) => {
                    self.log.push(LogEntry::StateChanged(StateChange::HpLost {
                        target,
                        amount: actual,
                    }));
                    let after = state
                        .creature(target)
                        .map(|creature| creature.hp)
                        .unwrap_or(0);
                    ApplyResult::Continue(vec![
                        Effect::Trigger(Event::CreatureHpChanged(CreatureHpChanged {
                            creature: target,
                            before: after + actual,
                            after,
                            source,
                        })),
                        Effect::CheckDeaths,
                    ])
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::Heal {
                target,
                amount,
                source: _,
            } => match state.heal(target, amount) {
                Ok(actual) => {
                    self.log.push(LogEntry::StateChanged(StateChange::Healed {
                        target,
                        amount: actual,
                    }));
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::GainMaxHp {
                target,
                amount,
                source: _,
            } => match state.gain_max_hp(target, amount) {
                Ok(actual) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::MaxHpGained {
                            target,
                            amount: actual,
                        }));
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::GainMaxHpIfFatal {
                target,
                beneficiary,
                amount,
                source: _,
            } => {
                let fatal = state
                    .creature(target)
                    .map(|creature| creature.hp <= 0)
                    .unwrap_or(false);
                if !fatal {
                    return ApplyResult::Continue(Vec::new());
                }
                match state.gain_max_hp(beneficiary, amount) {
                    Ok(actual) => {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::MaxHpGained {
                                target: beneficiary,
                                amount: actual,
                            }));
                        ApplyResult::Continue(Vec::new())
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::GainBlock {
                target,
                amount,
                source,
            } => {
                let calc = BlockCalc {
                    source,
                    target,
                    base_amount: amount,
                    amount,
                };
                let (calc, modifiers) = RulePipeline::modify_block(registry, state, calc);
                self.log
                    .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
                match state.gain_block(target, calc.amount) {
                    Ok(actual) => {
                        if matches!(source, Some(Source::Card(_))) && actual > 0 {
                            state.record_card_block_gained();
                        }
                        self.log
                            .push(LogEntry::StateChanged(StateChange::BlockGained {
                                target,
                                amount: actual,
                            }));
                        ApplyResult::Continue(vec![Effect::Trigger(Event::BlockGained(
                            BlockGained {
                                target,
                                amount: actual,
                                source,
                            },
                        ))])
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::ApplyPower {
                target,
                power,
                amount,
                source,
            } => match state.apply_power(target, power, amount) {
                Ok((instance, actual)) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::PowerApplied {
                            target,
                            power: instance,
                        }));
                    ApplyResult::Continue(vec![Effect::Trigger(Event::PowerApplied(
                        PowerApplied {
                            target,
                            power,
                            instance,
                            amount: actual,
                            source,
                        },
                    ))])
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::RemovePower { power } => match state.remove_power(power) {
                Ok(()) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::PowerRemoved { power }));
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::DrawCards { player, count } => {
                self.apply_draw_cards(state, registry, player, count, false)
            }
            Effect::DrawHandCards { player, count } => {
                self.apply_draw_cards(state, registry, player, count, true)
            }
            Effect::DrawUntilNonAttack { player } => {
                self.apply_draw_until_non_attack(state, registry, player)
            }
            Effect::DiscardHand { player } => match state.discard_hand(player) {
                Ok(cards) => {
                    for card in cards {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::CardMoved {
                                card,
                                from: Some(crate::core::state::PileId {
                                    owner: player,
                                    kind: PileKind::Hand,
                                }),
                                to: crate::core::state::PileId {
                                    owner: player,
                                    kind: PileKind::Discard,
                                },
                                reason: crate::core::effect::MoveReason::Discard,
                            }));
                    }
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::ExhaustCard { card } => self.apply_exhaust_card(state, registry, card),
            Effect::ExhaustTopDraw { player, count } => {
                let cards = state
                    .combat()
                    .filter(|combat| combat.player.id == player)
                    .map(|combat| {
                        combat
                            .player
                            .piles
                            .draw
                            .iter()
                            .rev()
                            .take(count as usize)
                            .copied()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                ApplyResult::Continue(
                    cards
                        .into_iter()
                        .map(|card| Effect::ExhaustCard { card })
                        .collect(),
                )
            }
            Effect::ExhaustRandomHand { player, filter } => {
                let cards = matching_hand_cards(state, registry, player, filter);
                let Some(index) = state.rng.combat_card_selection.next_usize(cards.len()) else {
                    return ApplyResult::Continue(Vec::new());
                };
                ApplyResult::Continue(vec![Effect::ExhaustCard { card: cards[index] }])
            }
            Effect::ExhaustHand { player, filter } => {
                let cards = matching_hand_cards(state, registry, player, filter);
                ApplyResult::Continue(
                    cards
                        .into_iter()
                        .map(|card| Effect::ExhaustCard { card })
                        .collect(),
                )
            }
            Effect::UpgradeCard { card } => match state.upgrade_card(card) {
                Ok(true) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::CardUpgraded { card }));
                    let player = state.card(card).map(|card| card.owner);
                    ApplyResult::Continue(
                        player
                            .map(|player| {
                                vec![Effect::Trigger(Event::CardUpgraded(CardUpgraded {
                                    player,
                                    card,
                                }))]
                            })
                            .unwrap_or_default(),
                    )
                }
                Ok(false) => ApplyResult::Continue(Vec::new()),
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::UpgradeHand { player, mode } => {
                let mut cards = matching_hand_cards(state, registry, player, CardFilter::Any)
                    .into_iter()
                    .filter(|card| {
                        state
                            .card(*card)
                            .map(|card| !card.upgraded)
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>();
                match mode {
                    UpgradeMode::All => {}
                    UpgradeMode::First => cards.truncate(1),
                    UpgradeMode::Random => {
                        if let Some(index) = state.rng.combat_card_selection.next_usize(cards.len())
                        {
                            cards = vec![cards[index]];
                        } else {
                            cards.clear();
                        }
                    }
                }
                ApplyResult::Continue(
                    cards
                        .into_iter()
                        .map(|card| Effect::UpgradeCard { card })
                        .collect(),
                )
            }
            Effect::AddCardCounter {
                card,
                counter,
                amount,
            } => match state.add_card_counter(card, counter, amount) {
                Ok(value) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::CardCounterChanged {
                            card,
                            counter,
                            value,
                        }));
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::AddGeneratedCard {
                player,
                def,
                to,
                upgraded,
                temporary,
                zero_cost_this_turn,
            } => {
                let costs = registry
                    .cards
                    .get(def)
                    .map(|def| def.costs_for(upgraded))
                    .unwrap_or_default();
                match state.add_generated_card(
                    player,
                    def,
                    to,
                    upgraded,
                    costs,
                    temporary,
                    zero_cost_this_turn,
                ) {
                    Ok(card) => {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::CardMoved {
                                card,
                                from: None,
                                to,
                                reason: MoveReason::Generated,
                            }));
                        ApplyResult::Continue(Vec::new())
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::GenerateRandomCardToHand {
                player,
                card_type,
                target,
                zero_cost_this_turn,
            } => {
                let candidates = registry
                    .cards
                    .values()
                    .filter(|def| def.can_generate_in_combat)
                    .filter(|def| {
                        card_type
                            .map(|card_type| def.card_type == card_type)
                            .unwrap_or(true)
                    })
                    .filter(|def| target.map(|target| def.target == target).unwrap_or(true))
                    .map(|def| def.id)
                    .collect::<Vec<_>>();
                let Some(index) = state
                    .rng
                    .combat_card_generation
                    .next_usize(candidates.len())
                else {
                    return ApplyResult::Continue(Vec::new());
                };
                ApplyResult::Continue(vec![Effect::AddGeneratedCard {
                    player,
                    def: candidates[index],
                    to: crate::core::state::PileId::player(player, PileKind::Hand),
                    upgraded: false,
                    temporary: true,
                    zero_cost_this_turn,
                }])
            }
            Effect::PlayTopDrawCards {
                player,
                count,
                exhaust_after_play,
            } => self.apply_play_top_draw_cards(state, registry, player, count, exhaust_after_play),
            Effect::ClearSideBlock(side) => {
                let targets = state
                    .combat()
                    .map(|combat| {
                        combat
                            .creatures
                            .iter()
                            .filter(|creature| creature.side == side && creature.alive)
                            .map(|creature| creature.id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                for target in targets {
                    let decision = RulePipeline::should_clear_block(registry, state, target);
                    self.log.push(LogEntry::DecisionMade(decision.clone()));
                    if !decision.is_allowed() {
                        continue;
                    }
                    match state.clear_block(target) {
                        Ok(amount) if amount > 0 => {
                            self.log
                                .push(LogEntry::StateChanged(StateChange::BlockCleared {
                                    target,
                                    amount,
                                }));
                        }
                        Ok(_) => {}
                        Err(error) => return ApplyResult::StateError(error),
                    }
                }
                ApplyResult::Continue(Vec::new())
            }
            Effect::ExecuteMonsterTurn => self.execute_monster_turn(state, registry),
            Effect::MoveCard { card, to, reason } => match state.move_card(card, to) {
                Ok(from_kind) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::CardMoved {
                            card,
                            from: from_kind.map(|kind| crate::core::state::PileId {
                                owner: to.owner,
                                kind,
                            }),
                            to,
                            reason,
                        }));
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::CheckDeaths => self.check_deaths(state, registry),
            Effect::CheckCombatEnd => combat_result_for_state(state)
                .map(ApplyResult::CombatOver)
                .unwrap_or_else(|| ApplyResult::Continue(Vec::new())),
            Effect::StartTurn(side) => {
                let phase = match side {
                    Side::Player => CombatPhase::PlayerStart,
                    Side::Monsters => CombatPhase::EnemyAction,
                };
                match state.set_phase(phase) {
                    Ok(()) => {
                        if side == Side::Player {
                            state.reset_turn_stats();
                        }
                        ApplyResult::Continue(start_turn_effects(state, side))
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::EndTurn(side) => {
                let phase = match side {
                    Side::Player => CombatPhase::PlayerEnd,
                    Side::Monsters => CombatPhase::EnemyEnd,
                };
                match state.set_phase(phase) {
                    Ok(()) => ApplyResult::Continue(end_turn_effects(state, side)),
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::EnterPhase(phase) => match state.set_phase(phase) {
                Ok(()) => ApplyResult::Continue(Vec::new()),
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::RequestChoice(choice) => ApplyResult::NeedChoice(choice),
            Effect::ResolveChoice(_) => ApplyResult::Continue(Vec::new()),
        }
    }

    fn apply_damage(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        op: DamageOp,
    ) -> ApplyResult {
        let calc = DamageCalc {
            source: op.source,
            dealer: op.dealer,
            target: op.target,
            kind: op.kind,
            base_amount: op.base_amount,
            amount: op.base_amount,
        };
        let (calc, modifiers) = RulePipeline::modify_damage(registry, state, calc);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));

        let target = op.target;
        let Some(creature) = state.creature_mut(target) else {
            return ApplyResult::StateError(StateError::UnknownCreature(target));
        };

        let requested = if calc.amount < Decimal::from(0) {
            Decimal::from(0)
        } else {
            calc.amount
        };

        let hp_before = creature.hp;
        let mut blocked = 0;
        let mut hp_loss = requested;

        if !op.flags.ignores_block {
            let blocked_decimal = if Decimal::from(creature.block) < requested {
                Decimal::from(creature.block)
            } else {
                requested
            };
            blocked = decimal_to_i32_trunc(blocked_decimal);
            creature.block = creature.block.saturating_sub(blocked);
            hp_loss = requested - blocked_decimal;
        }

        let hp_loss_int = decimal_to_i32_trunc(hp_loss);
        creature.hp = creature.hp.saturating_sub(hp_loss_int).max(0);
        let hp_after = creature.hp;
        let actual_hp_loss = hp_before - hp_after;
        let is_player = state.player_creature_id() == Some(target);
        if is_player && actual_hp_loss > 0 {
            state.record_player_hp_loss(actual_hp_loss);
        }

        let result = DamageResult {
            source: op.source,
            dealer: op.dealer,
            target,
            kind: op.kind,
            requested,
            blocked,
            hp_loss: actual_hp_loss,
        };

        self.log
            .push(LogEntry::StateChanged(StateChange::DamageApplied(
                result.clone(),
            )));

        ApplyResult::Continue(vec![
            Effect::Trigger(Event::CreatureHpChanged(CreatureHpChanged {
                creature: target,
                before: hp_before,
                after: hp_after,
                source: op.source,
            })),
            Effect::Trigger(Event::DamageDealt(result)),
            Effect::CheckDeaths,
        ])
    }

    fn execute_monster_turn(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
    ) -> ApplyResult {
        let monsters = state
            .combat()
            .map(|combat| {
                combat
                    .creatures
                    .iter()
                    .filter(|creature| creature.side == Side::Monsters && creature.alive)
                    .map(|creature| creature.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for monster in monsters {
            let Some(effects) = monster_action_effects(state, registry, monster) else {
                continue;
            };

            if let Err(error) = state.increment_turns_taken(monster) {
                return ApplyResult::StateError(error);
            }

            match self.apply_immediate_effects(state, registry, effects) {
                ApplyResult::Continue(_) => {}
                result => return result,
            }

            if let Some(result) = combat_result_for_state(state) {
                return ApplyResult::CombatOver(result);
            }
        }

        ApplyResult::Continue(vec![Effect::EndTurn(Side::Monsters)])
    }

    fn apply_draw_cards(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: crate::core::ids::PlayerId,
        count: u8,
        from_hand_draw: bool,
    ) -> ApplyResult {
        if count == 0 {
            return ApplyResult::Continue(Vec::new());
        }

        let decision = RulePipeline::should_draw(registry, state, player, from_hand_draw);
        self.log.push(LogEntry::DecisionMade(decision.clone()));
        if !decision.is_allowed() {
            return ApplyResult::Continue(Vec::new());
        }

        for _ in 0..count {
            match state.shuffle_discard_into_draw_if_needed(player) {
                Ok(Some(cards)) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::CardsShuffled {
                            player,
                            cards: cards.clone(),
                        }));
                    let event = Event::CardsShuffled(CardsShuffled { player, cards });
                    match self.apply_immediate_effects(
                        state,
                        registry,
                        vec![Effect::Trigger(event)],
                    ) {
                        ApplyResult::Continue(_) => {}
                        other => return other,
                    }
                }
                Ok(None) => {}
                Err(error) => return ApplyResult::StateError(error),
            }

            let card = match state.draw_one_card(player) {
                Ok(Some(card)) => card,
                Ok(None) => break,
                Err(error) => return ApplyResult::StateError(error),
            };

            self.log
                .push(LogEntry::StateChanged(StateChange::CardMoved {
                    card,
                    from: Some(crate::core::state::PileId {
                        owner: player,
                        kind: PileKind::Draw,
                    }),
                    to: crate::core::state::PileId {
                        owner: player,
                        kind: PileKind::Hand,
                    },
                    reason: crate::core::effect::MoveReason::Draw,
                }));
            match self.apply_immediate_effects(
                state,
                registry,
                vec![Effect::Trigger(Event::CardDrawn(CardDrawn {
                    player,
                    card,
                    from_hand_draw,
                }))],
            ) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
        }

        ApplyResult::Continue(Vec::new())
    }

    fn apply_immediate_effects(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        effects: Vec<Effect>,
    ) -> ApplyResult {
        let mut local = VecDeque::from(effects);
        while let Some(effect) = local.pop_front() {
            self.log.push(LogEntry::EffectStarted(effect.clone()));
            match self.apply_effect(state, registry, effect) {
                ApplyResult::Continue(more) => local.extend(more),
                other => return other,
            }
        }
        ApplyResult::Continue(Vec::new())
    }

    fn record_event_stats(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        event: &Event,
    ) {
        if let Event::CardPlayStarted(event) = event {
            let is_attack = state
                .card(event.card)
                .and_then(|card| registry.cards.get(card.def))
                .map(|def| def.card_type == CardType::Attack)
                .unwrap_or(false);
            if is_attack {
                state.record_attack_played();
            }
        }
    }

    fn apply_exhaust_card(
        &mut self,
        state: &mut GameState,
        _registry: &StaticRegistry,
        card: crate::core::ids::CardInstanceId,
    ) -> ApplyResult {
        let Some(player) = state.card(card).map(|card| card.owner) else {
            return ApplyResult::StateError(StateError::UnknownCard(card));
        };
        match state.exhaust_card(card) {
            Ok(from_kind) => {
                state.record_card_exhausted();
                let to = crate::core::state::PileId::player(player, PileKind::Exhaust);
                self.log
                    .push(LogEntry::StateChanged(StateChange::CardMoved {
                        card,
                        from: from_kind.map(|kind| crate::core::state::PileId {
                            owner: player,
                            kind,
                        }),
                        to,
                        reason: MoveReason::Exhaust,
                    }));
                ApplyResult::Continue(vec![Effect::Trigger(Event::CardExhausted(CardExhausted {
                    player,
                    card,
                    source: None,
                }))])
            }
            Err(error) => ApplyResult::StateError(error),
        }
    }

    fn apply_draw_until_non_attack(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: crate::core::ids::PlayerId,
    ) -> ApplyResult {
        loop {
            let before = state
                .combat()
                .map(|combat| combat.player.piles.hand.len())
                .unwrap_or_default();
            match self.apply_draw_cards(state, registry, player, 1, false) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
            let Some(card) = state
                .combat()
                .and_then(|combat| combat.player.piles.hand.last().copied())
            else {
                return ApplyResult::Continue(Vec::new());
            };
            let after = state
                .combat()
                .map(|combat| combat.player.piles.hand.len())
                .unwrap_or_default();
            if after == before {
                return ApplyResult::Continue(Vec::new());
            }
            let is_attack = state
                .card(card)
                .and_then(|card| registry.cards.get(card.def))
                .map(|def| def.card_type == CardType::Attack)
                .unwrap_or(false);
            if !is_attack {
                return ApplyResult::Continue(Vec::new());
            }
        }
    }

    fn apply_play_top_draw_cards(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: crate::core::ids::PlayerId,
        count: u8,
        exhaust_after_play: bool,
    ) -> ApplyResult {
        let cards = state
            .combat()
            .filter(|combat| combat.player.id == player)
            .map(|combat| {
                combat
                    .player
                    .piles
                    .draw
                    .iter()
                    .rev()
                    .take(count as usize)
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let target = state.alive_monster_ids().first().copied();
        for card in cards {
            let play = crate::core::state::PileId::player(player, PileKind::Play);
            let result = if exhaust_after_play {
                (
                    crate::core::state::PileId::player(player, PileKind::Exhaust),
                    MoveReason::Exhaust,
                )
            } else {
                card_auto_result_pile(state, registry, player, card)
            };
            let effects = vec![
                Effect::MoveCard {
                    card,
                    to: play,
                    reason: MoveReason::Play,
                },
                Effect::Trigger(Event::CardPlayStarted(
                    crate::core::event::CardPlayStarted {
                        player,
                        card,
                        target,
                    },
                )),
                Effect::ExecuteCardBody {
                    player,
                    card,
                    target,
                },
                Effect::MoveCard {
                    card,
                    to: result.0,
                    reason: result.1,
                },
                Effect::Trigger(Event::CardPlayed(crate::core::event::CardPlayed {
                    player,
                    card,
                    target,
                })),
            ];
            match self.apply_immediate_effects(state, registry, effects) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
        }
        ApplyResult::Continue(Vec::new())
    }

    fn resolve_card_payment(
        &mut self,
        state: &GameState,
        registry: &StaticRegistry,
        player: crate::core::ids::PlayerId,
        card: crate::core::ids::CardInstanceId,
        log_modifiers: bool,
    ) -> Result<CardPayment, CommandError> {
        let card_state = state.card(card).ok_or(CommandError::InvalidCard(card))?;
        let def_costs = registry
            .cards
            .get(card_state.def)
            .map(|def| def.costs_for(card_state.upgraded))
            .unwrap_or(card_state.costs);
        let costs = card_state.costs_with_temporary(def_costs);

        Ok(CardPayment {
            energy: self.resolve_resource_cost(
                state,
                player,
                card,
                registry,
                ResourceKind::Energy,
                costs.energy,
                log_modifiers,
            )?,
            stars: self.resolve_resource_cost(
                state,
                player,
                card,
                registry,
                ResourceKind::Stars,
                costs.stars,
                log_modifiers,
            )?,
        })
    }

    fn resolve_resource_cost(
        &mut self,
        state: &GameState,
        player: crate::core::ids::PlayerId,
        card: crate::core::ids::CardInstanceId,
        registry: &StaticRegistry,
        resource: ResourceKind,
        cost: CardCost,
        log_modifiers: bool,
    ) -> Result<i32, CommandError> {
        let available = state
            .resource_amount(player, resource)
            .map_err(command_error_from_state)?;
        let Some(mut amount) = cost.amount_to_pay(available) else {
            return Err(CommandError::Prevented(PreventReason::CannotPlay));
        };

        if matches!(cost, CardCost::Fixed(_)) {
            let calc = ResourceCostCalc {
                player,
                card,
                resource,
                base_cost: amount,
                cost: amount,
            };
            let (calc, modifiers) = RulePipeline::modify_resource_cost(registry, state, calc);
            if log_modifiers {
                self.log
                    .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
            }
            amount = calc.cost.max(0);
        }

        if amount > available {
            return Err(CommandError::Prevented(
                PreventReason::InsufficientResource(resource),
            ));
        }

        Ok(amount)
    }

    fn spend_card_payment(
        &mut self,
        state: &mut GameState,
        player: crate::core::ids::PlayerId,
        payment: CardPayment,
    ) -> ApplyResult {
        let mut effects = Vec::new();
        for (resource, amount) in [
            (ResourceKind::Energy, payment.energy),
            (ResourceKind::Stars, payment.stars),
        ] {
            if amount == 0 {
                continue;
            }
            match state.spend_resource(player, resource, amount) {
                Ok(()) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::ResourceSpent {
                            player,
                            resource,
                            amount,
                        }));
                    effects.push(Effect::Trigger(Event::ResourceSpent(ResourceChanged {
                        player,
                        resource,
                        amount,
                    })));
                }
                Err(StateError::NotEnoughResource { resource, .. }) => {
                    return ApplyResult::Rejected(CommandError::Prevented(
                        PreventReason::InsufficientResource(resource),
                    ));
                }
                Err(error) => return ApplyResult::StateError(error),
            }
        }
        ApplyResult::Continue(effects)
    }

    fn check_deaths(&mut self, state: &mut GameState, registry: &StaticRegistry) -> ApplyResult {
        let mut effects = Vec::new();
        for creature in state.death_candidates() {
            let decision = RulePipeline::should_die(registry, state, creature);
            self.log.push(LogEntry::DecisionMade(decision.clone()));
            match decision {
                Decision::Allow => match state.mark_dead(creature) {
                    Ok(()) => {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::CreatureDied {
                                creature,
                            }));
                        effects.push(Effect::Trigger(Event::CreatureDied { creature }));
                    }
                    Err(error) => return ApplyResult::StateError(error),
                },
                Decision::Prevent { .. } => {
                    effects.push(Effect::Trigger(Event::DeathPrevented { creature }));
                }
            }
        }

        if let Some(result) = combat_result_for_state(state) {
            return ApplyResult::CombatOver(result);
        }

        ApplyResult::Continue(effects)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CardPayment {
    energy: i32,
    stars: i32,
}

fn matching_hand_cards(
    state: &GameState,
    registry: &StaticRegistry,
    player: crate::core::ids::PlayerId,
    filter: CardFilter,
) -> Vec<crate::core::ids::CardInstanceId> {
    let Some(combat) = state.combat().filter(|combat| combat.player.id == player) else {
        return Vec::new();
    };
    combat
        .player
        .piles
        .hand
        .iter()
        .copied()
        .filter(|card| match filter {
            CardFilter::Any => true,
            CardFilter::Attack => state
                .card(*card)
                .and_then(|card| registry.cards.get(card.def))
                .map(|def| def.card_type == CardType::Attack)
                .unwrap_or(false),
            CardFilter::NonAttack => state
                .card(*card)
                .and_then(|card| registry.cards.get(card.def))
                .map(|def| def.card_type != CardType::Attack)
                .unwrap_or(false),
        })
        .collect()
}

fn card_auto_result_pile(
    state: &GameState,
    registry: &StaticRegistry,
    player: crate::core::ids::PlayerId,
    card: crate::core::ids::CardInstanceId,
) -> (crate::core::state::PileId, MoveReason) {
    let Some(card_state) = state.card(card) else {
        return (
            crate::core::state::PileId::player(player, PileKind::Discard),
            MoveReason::Discard,
        );
    };
    let Some(def) = registry.cards.get(card_state.def) else {
        return (
            crate::core::state::PileId::player(player, PileKind::Discard),
            MoveReason::Discard,
        );
    };
    if def.card_type == CardType::Power || card_state.flags.purge_on_use {
        (
            crate::core::state::PileId::player(player, PileKind::Removed),
            MoveReason::Removed,
        )
    } else if def.has_keyword(card_state.upgraded, CardKeyword::Exhaust) {
        (
            crate::core::state::PileId::player(player, PileKind::Exhaust),
            MoveReason::Exhaust,
        )
    } else {
        (
            crate::core::state::PileId::player(player, PileKind::Discard),
            MoveReason::Discard,
        )
    }
}

fn command_error_from_state(error: StateError) -> CommandError {
    match error {
        StateError::CombatNotActive => CommandError::CombatRequired,
        _ => CommandError::Prevented(PreventReason::CannotPlay),
    }
}

fn validate_card_target(
    state: &GameState,
    registry: &StaticRegistry,
    card: crate::core::ids::CardInstanceId,
    target: Option<CreatureId>,
) -> Result<(), PreventReason> {
    let Some(card_state) = state.card(card) else {
        return Err(PreventReason::CannotPlay);
    };
    let Some(def) = registry.cards.get(card_state.def) else {
        return Ok(());
    };

    match def.target {
        TargetType::None => {
            if target.is_none() {
                Ok(())
            } else {
                Err(PreventReason::NoValidTarget)
            }
        }
        TargetType::Enemy => {
            let Some(target) = target else {
                return Err(PreventReason::NoValidTarget);
            };
            match state.creature(target) {
                Some(creature) if creature.side == Side::Monsters && creature.alive => Ok(()),
                _ => Err(PreventReason::NoValidTarget),
            }
        }
        TargetType::AllEnemies | TargetType::RandomEnemy => Ok(()),
        TargetType::SelfTarget => {
            if target.is_none() || target == state.player_creature_id() {
                Ok(())
            } else {
                Err(PreventReason::NoValidTarget)
            }
        }
        TargetType::AnyAlly => {
            if target.is_none() || target == state.player_creature_id() {
                return Ok(());
            }
            let Some(target) = target else {
                return Err(PreventReason::NoValidTarget);
            };
            match state.creature(target) {
                Some(creature) if creature.side == Side::Player && creature.alive => Ok(()),
                _ => Err(PreventReason::NoValidTarget),
            }
        }
        TargetType::AnyCreature => {
            let Some(target) = target else {
                return Err(PreventReason::NoValidTarget);
            };
            match state.creature(target) {
                Some(creature) if creature.alive => Ok(()),
                _ => Err(PreventReason::NoValidTarget),
            }
        }
    }
}

fn start_turn_effects(state: &GameState, side: Side) -> Vec<Effect> {
    let mut effects = vec![
        Effect::Trigger(Event::TurnStarted { side }),
        Effect::ClearSideBlock(side),
    ];

    match side {
        Side::Player => {
            if let Some(combat) = state.combat() {
                let player = combat.player.id;
                let diff = combat.player.max_energy - combat.player.energy;
                if diff > 0 {
                    effects.push(Effect::GainResource {
                        player,
                        resource: ResourceKind::Energy,
                        amount: diff,
                    });
                } else if diff < 0 {
                    effects.push(Effect::SpendResource {
                        player,
                        resource: ResourceKind::Energy,
                        amount: -diff,
                    });
                }
                effects.push(Effect::DrawHandCards {
                    player,
                    count: BASE_HAND_DRAW_COUNT,
                });
            }
            effects.push(Effect::EnterPhase(CombatPhase::PlayerAction));
        }
        Side::Monsters => effects.push(Effect::ExecuteMonsterTurn),
    }

    effects
}

fn end_turn_effects(state: &GameState, side: Side) -> Vec<Effect> {
    match side {
        Side::Player => state
            .player_id()
            .map(|player| {
                vec![
                    Effect::DiscardHand { player },
                    Effect::CheckCombatEnd,
                    Effect::StartTurn(Side::Monsters),
                ]
            })
            .unwrap_or_default(),
        Side::Monsters => vec![
            Effect::Trigger(Event::TurnEnded {
                side: Side::Monsters,
            }),
            Effect::CheckCombatEnd,
            Effect::StartTurn(Side::Player),
        ],
    }
}

fn monster_action_effects(
    state: &GameState,
    registry: &StaticRegistry,
    monster: CreatureId,
) -> Option<Vec<Effect>> {
    let creature = state.creature(monster)?;
    if !creature.alive {
        return None;
    }
    let model = creature.model?;
    let def = registry.monsters.get(model)?;
    let ctx = RuleCtx {
        state,
        registry,
        listener: Some(ListenerRef::Monster(monster)),
    };
    Some((def.act)(&ctx, monster))
}

enum ApplyResult {
    Continue(Vec<Effect>),
    NeedChoice(crate::core::effect::ChoiceRequest),
    CombatOver(crate::core::engine::CombatResult),
    Rejected(CommandError),
    StateError(StateError),
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::content::cards::{
        CardDef, CardPlayCtx, CardRarity, CardRules, CardType, TargetType,
    };
    use crate::content::powers::{PowerDef, PowerRules};
    use crate::core::effect::{DamageFlags, DamageKind, DamageOp, Effect, Source};
    use crate::core::engine::{CombatOutcome, Engine, StepResult};
    use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey, PowerId, PowerInstanceId};
    use crate::core::query::{
        DamageCalc, Decision, DecisionQuery, DecisionQueryKind, PreventReason,
    };
    use crate::core::rules::{prevent_by_current_listener, RuleCtx};
    use crate::core::state::{CardCost, CardCosts, CombatSetupCard, CombatSetupMonster, GameState};
    use crate::core::Command;
    use crate::registry::StaticRegistry;

    use super::EffectResolver;

    const STARTER_STRIKE: CardId = CardId::new("starter_strike");
    const TEST_STRENGTH: PowerId = PowerId::new("test_strength");
    const TEST_CANNOT_DIE: PowerId = PowerId::new("test_cannot_die");

    fn test_strike_body(
        ctx: &CardPlayCtx<'_>,
        card: CardInstanceId,
        target: Option<CreatureId>,
    ) -> Vec<Effect> {
        let Some(target) = target else {
            return Vec::new();
        };

        vec![Effect::DealDamage(DamageOp {
            source: Some(Source::Card(card)),
            dealer: ctx.state.player_creature_id(),
            target,
            base_amount: Decimal::from(6),
            kind: DamageKind::Attack,
            flags: DamageFlags {
                ignores_block: false,
                is_attack: true,
            },
        })]
    }

    fn test_strike_def() -> CardDef {
        CardDef {
            id: STARTER_STRIKE,
            loc_key: LocKey::new("card.starter_strike"),
            card_type: CardType::Attack,
            rarity: CardRarity::Basic,
            target: TargetType::Enemy,
            base_costs: CardCosts::energy(1),
            upgraded_costs: None,
            keywords: &[],
            upgraded_keywords: &[],
            tags: &[],
            can_generate_in_combat: true,
            play: test_strike_body,
            rules: CardRules::default(),
        }
    }

    fn strength_additive(
        ctx: &RuleCtx<'_>,
        power: PowerInstanceId,
        mut calc: DamageCalc,
    ) -> DamageCalc {
        let amount = ctx
            .state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
            .map(|instance| Decimal::from(instance.amount))
            .unwrap_or_default();
        calc.amount += amount;
        calc
    }

    fn strength_def() -> PowerDef {
        PowerDef {
            id: TEST_STRENGTH,
            loc_key: LocKey::new("power.test_strength"),
            rules: PowerRules {
                modify_damage_additive: Some(strength_additive),
                ..PowerRules::default()
            },
        }
    }

    fn prevent_owner_death(
        ctx: &RuleCtx<'_>,
        power: PowerInstanceId,
        query: &DecisionQuery,
    ) -> Decision {
        let DecisionQueryKind::ShouldDie { creature } = query.kind else {
            return Decision::Allow;
        };
        let owns_power = ctx
            .state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
            .map(|instance| instance.owner == creature)
            .unwrap_or(false);

        if owns_power {
            prevent_by_current_listener(ctx, PreventReason::CannotDie)
        } else {
            Decision::Allow
        }
    }

    fn cannot_die_def() -> PowerDef {
        PowerDef {
            id: TEST_CANNOT_DIE,
            loc_key: LocKey::new("power.test_cannot_die"),
            rules: PowerRules {
                decide: Some(prevent_owner_death),
                ..PowerRules::default()
            },
        }
    }

    #[test]
    fn damage_effect_can_end_combat() {
        let mut state = GameState::demo_combat(7);
        let target = state.combat().unwrap().monster_ids()[0];
        let mut resolver = EffectResolver::default();

        resolver.enqueue(Effect::DealDamage(DamageOp {
            source: None,
            dealer: state.player_creature_id(),
            target,
            base_amount: Decimal::from(99),
            kind: DamageKind::Attack,
            flags: DamageFlags {
                ignores_block: false,
                is_attack: true,
            },
        }));
        resolver.enqueue(Effect::CheckCombatEnd);

        let registry = StaticRegistry::default();
        match resolver.drain(&mut state, &registry) {
            StepResult::CombatOver(result, _) => {
                assert_eq!(result.outcome, CombatOutcome::Victory);
            }
            other => panic!("expected combat over, got {other:?}"),
        }
    }

    #[test]
    fn lethal_damage_to_player_ends_combat_as_defeat() {
        let mut state = GameState::full_nibbit_combat(8);
        let player = state.player_creature_id().unwrap();
        let enemy = state.combat().unwrap().monster_ids()[0];
        let mut resolver = EffectResolver::default();

        resolver.enqueue(Effect::DealDamage(DamageOp {
            source: Some(Source::Creature(enemy)),
            dealer: Some(enemy),
            target: player,
            base_amount: Decimal::from(99),
            kind: DamageKind::Attack,
            flags: DamageFlags {
                ignores_block: false,
                is_attack: true,
            },
        }));
        resolver.enqueue(Effect::CheckCombatEnd);

        let registry = StaticRegistry::default();
        match resolver.drain(&mut state, &registry) {
            StepResult::CombatOver(result, _) => {
                assert_eq!(result.outcome, CombatOutcome::Defeat);
            }
            other => panic!("expected combat over, got {other:?}"),
        }
    }

    #[test]
    fn playing_card_can_spend_energy_and_stars() {
        let mut engine = Engine::new(GameState::demo_combat(11));
        let player = engine.state.player_id().unwrap();
        let combat = engine.state.combat_mut().unwrap();
        combat.player.stars = 3;
        let card = combat.player.piles.hand[0];
        combat.cards.get_mut(&card).unwrap().costs.stars = CardCost::Fixed(2);
        let target = combat.monster_ids()[0];

        let result = engine.step(Command::PlayCard {
            player,
            card,
            target: Some(target),
        });

        assert!(matches!(result, StepResult::Done(_)));
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.energy, 2);
        assert_eq!(combat.player.stars, 1);
        assert!(combat.player.piles.discard.contains(&card));
    }

    #[test]
    fn card_play_is_rejected_when_stars_are_short() {
        let mut engine = Engine::new(GameState::demo_combat(12));
        let player = engine.state.player_id().unwrap();
        let combat = engine.state.combat_mut().unwrap();
        let card = combat.player.piles.hand[0];
        combat.cards.get_mut(&card).unwrap().costs.stars = CardCost::Fixed(2);
        let target = combat.monster_ids()[0];

        let result = engine.step(Command::PlayCard {
            player,
            card,
            target: Some(target),
        });

        assert!(matches!(result, StepResult::Rejected(_, _)));
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.energy, 3);
        assert_eq!(combat.player.stars, 0);
        assert!(combat.player.piles.hand.contains(&card));
    }

    #[test]
    fn card_body_is_resolved_through_registry() {
        let mut registry = StaticRegistry::default();
        registry.cards.register(test_strike_def());

        let mut engine = Engine::with_registry(GameState::demo_combat(13), registry);
        let player = engine.state.player_id().unwrap();
        let card = engine.state.combat().unwrap().player.piles.hand[0];
        let target = engine.state.combat().unwrap().monster_ids()[0];

        let result = engine.step(Command::PlayCard {
            player,
            card,
            target: Some(target),
        });

        assert!(matches!(result, StepResult::Done(_)));
        let enemy = engine.state.creature(target).unwrap();
        assert_eq!(enemy.hp, 24);
    }

    #[test]
    fn draw_cards_stops_at_sts2_hand_limit() {
        let setup = CombatSetupCard {
            def: STARTER_STRIKE,
            upgraded: false,
            costs: CardCosts::energy(1),
        };
        let mut state = GameState::single_player_test_combat(
            16,
            [setup; 12],
            [CombatSetupMonster {
                model: None,
                max_hp: 10,
            }],
            50,
            3,
            10,
        );
        let player = state.player_id().unwrap();
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::DrawCards { player, count: 2 });

        let result = resolver.drain(&mut state, &StaticRegistry::empty());

        assert!(matches!(result, StepResult::Done(_)));
        let combat = state.combat().unwrap();
        assert_eq!(combat.player.piles.hand.len(), 10);
        assert_eq!(combat.player.piles.draw.len(), 2);
    }

    #[test]
    fn power_rule_can_modify_damage() {
        let mut registry = StaticRegistry::default();
        registry.cards.register(test_strike_def());
        registry.powers.register(strength_def());

        let mut engine = Engine::with_registry(GameState::demo_combat(14), registry);
        let player = engine.state.player_id().unwrap();
        let player_creature = engine.state.player_creature_id().unwrap();
        engine
            .state
            .apply_power(player_creature, TEST_STRENGTH, Decimal::from(2))
            .unwrap();

        let card = engine.state.combat().unwrap().player.piles.hand[0];
        let target = engine.state.combat().unwrap().monster_ids()[0];

        let result = engine.step(Command::PlayCard {
            player,
            card,
            target: Some(target),
        });

        let StepResult::Done(log) = result else {
            panic!("expected card play to finish");
        };
        assert!(log
            .iter()
            .any(|entry| matches!(entry, crate::core::log::LogEntry::ModifierApplied(_))));
        let enemy = engine.state.creature(target).unwrap();
        assert_eq!(enemy.hp, 22);
    }

    #[test]
    fn decision_rule_can_prevent_death() {
        let mut registry = StaticRegistry::default();
        registry.powers.register(cannot_die_def());

        let mut state = GameState::demo_combat(15);
        let target = state.combat().unwrap().monster_ids()[0];
        state
            .apply_power(target, TEST_CANNOT_DIE, Decimal::from(1))
            .unwrap();

        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::DealDamage(DamageOp {
            source: None,
            dealer: state.player_creature_id(),
            target,
            base_amount: Decimal::from(99),
            kind: DamageKind::Attack,
            flags: DamageFlags {
                ignores_block: false,
                is_attack: true,
            },
        }));

        let result = resolver.drain(&mut state, &registry);

        assert!(matches!(result, StepResult::Done(_)));
        let creature = state.creature(target).unwrap();
        assert!(creature.alive);
        assert!(creature.hp == 0);
    }
}
