use std::collections::{BTreeSet, VecDeque};

use rust_decimal::Decimal;

use crate::content::cards::{CardKeyword, CardPlayCtx, CardType, TargetType};
use crate::core::command::CommandError;
use crate::core::effect::{
    AutoPlayReason, CardFilter, ChoiceAction, ChoiceKind, ChoiceOption, ChoiceResolution,
    ChoiceResponse, ChoiceValue, DamageOp, DamageResult, DiscardKind, Effect, MoveReason,
    OrbSelection, OrbTrigger, Source, UpgradeMode,
};
use crate::core::engine::{combat_result_for_state, StepResult};
use crate::core::event::{
    BlockGained, CardDiscarded, CardDrawn, CardExhausted, CardPlayed, CardUpgraded, CardsShuffled,
    CreatureHpChanged, Event, OrbChanneled, OrbEvoked, PowerAmountChanged, PowerApplied,
    ResourceChanged, Summoned,
};
use crate::core::ids::{
    CardId, CardInstanceId, ChoiceId, CreatureId, LocKey, OrbInstanceId, PlayerId,
};
use crate::core::listener::ListenerRef;
use crate::core::log::{LogEntry, StateChange};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, CardPlayResultPileModifierLog, DamageCalc, Decision,
    HpLossCalc, HpLossPhase, OrbPassiveTriggerCountCalc, PilePosition, PowerAmountCalc,
    PowerAmountPhase, PreventReason, ResourceCostCalc, SummonAmountCalc, UnblockedDamageTargetCalc,
};
use crate::core::rules::{RuleCtx, RulePipeline};
use crate::core::state::{
    decimal_to_i32_trunc, CardCost, CardKeywordDuration, CombatPhase, GameState, PileId, PileKind,
    PlayerPetKind, ResourceKind, Side, StateError, BASE_HAND_DRAW_COUNT,
};
use crate::registry::StaticRegistry;

#[derive(Default)]
pub struct EffectResolver {
    queue: VecDeque<Effect>,
    pending_choice: Option<crate::core::effect::ChoiceRequest>,
    log: Vec<LogEntry>,
    last_card_payment: std::collections::BTreeMap<crate::core::ids::CardInstanceId, CardPayment>,
    pending_card_results:
        std::collections::BTreeMap<crate::core::ids::CardInstanceId, CardPlayResult>,
    next_choice: u32,
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

    pub fn pending_choice(&self) -> Option<&crate::core::effect::ChoiceRequest> {
        self.pending_choice.as_ref()
    }

    pub fn submit_choice(&mut self, response: ChoiceResponse) -> Result<(), CommandError> {
        let pending = self
            .pending_choice
            .as_ref()
            .ok_or(CommandError::UnexpectedChoice)?;
        if pending.id != response.request {
            return Err(CommandError::ChoiceMismatch {
                expected: pending.id,
                actual: response.request,
            });
        }

        let actual = response.options.len();
        if actual < pending.min || actual > pending.max {
            return Err(CommandError::ChoiceCountOutOfRange {
                min: pending.min,
                max: pending.max,
                actual,
            });
        }

        let mut seen = BTreeSet::new();
        let mut selected = Vec::with_capacity(response.options.len());
        for option_id in response.options {
            if !seen.insert(option_id) {
                return Err(CommandError::DuplicateChoiceOption(option_id));
            }
            let Some(option) = pending.options.iter().find(|option| option.id == option_id) else {
                return Err(CommandError::InvalidChoiceOption(option_id));
            };
            if !option.enabled {
                return Err(CommandError::DisabledChoiceOption(option_id));
            }
            selected.push(option.clone());
        }

        let request = self
            .pending_choice
            .take()
            .expect("pending choice was validated");
        self.queue
            .push_front(Effect::ResolveChoice(ChoiceResolution {
                request,
                selected,
            }));
        Ok(())
    }

    pub fn take_log(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.log)
    }

    fn prepare_choice(
        &mut self,
        mut choice: crate::core::effect::ChoiceRequest,
    ) -> crate::core::effect::ChoiceRequest {
        if choice.id.get() == 0 {
            self.next_choice += 1;
            choice.id = ChoiceId::new(self.next_choice);
        }
        choice
    }

    fn clear_aborted_resolution(&mut self) {
        self.queue.clear();
        self.last_card_payment.clear();
        self.pending_card_results.clear();
    }

    pub fn drain(&mut self, state: &mut GameState, registry: &StaticRegistry) -> StepResult {
        while let Some(effect) = self.queue.pop_front() {
            self.log.push(LogEntry::EffectStarted(effect.clone()));

            match self.apply_effect(state, registry, effect) {
                ApplyResult::Continue(more) => self.queue.extend(more),
                ApplyResult::NeedChoice(choice) => {
                    let choice = self.prepare_choice(choice);
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
            Effect::Trigger(event) => self.trigger_event(state, registry, event),
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
            Effect::AutoPlayCard {
                player,
                card,
                target,
                force_exhaust,
                reason: _,
            } => self.auto_play_card(state, registry, player, card, target, force_exhaust),
            Effect::PrepareCardPlayResult {
                player,
                card,
                force_exhaust,
            } => self.prepare_card_play_result(state, registry, player, card, force_exhaust),
            Effect::FinishCardPlay {
                player,
                card,
                target,
                force_exhaust,
            } => self.finish_card_play(state, registry, player, card, target, force_exhaust),
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
            } => {
                let giver = source_creature(state, source);
                let calc = PowerAmountCalc {
                    source,
                    giver,
                    target,
                    power,
                    base_amount: amount,
                    amount,
                    phase: PowerAmountPhase::Given,
                };
                let (calc, given_modifiers) =
                    RulePipeline::modify_power_amount(registry, state, calc);
                self.log
                    .extend(given_modifiers.into_iter().map(LogEntry::ModifierApplied));
                let calc = PowerAmountCalc {
                    phase: PowerAmountPhase::Received,
                    base_amount: calc.amount,
                    ..calc
                };
                let (calc, received_modifiers) =
                    RulePipeline::modify_power_amount(registry, state, calc);
                self.log.extend(
                    received_modifiers
                        .into_iter()
                        .map(LogEntry::ModifierApplied),
                );
                if calc.amount == Decimal::from(0) {
                    return ApplyResult::Continue(Vec::new());
                }
                match state.apply_power(target, power, calc.amount) {
                    Ok((instance, actual)) => {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::PowerApplied {
                                target,
                                power: instance,
                            }));
                        ApplyResult::Continue(vec![
                            Effect::Trigger(Event::PowerApplied(PowerApplied {
                                target,
                                power,
                                instance,
                                amount: actual,
                                source,
                            })),
                            Effect::Trigger(Event::PowerAmountChanged(PowerAmountChanged {
                                target,
                                power,
                                instance,
                                delta: actual,
                                source,
                            })),
                        ])
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::ApplyPowerToRandomEnemy {
                power,
                amount,
                source,
                count,
            } => {
                let mut effects = Vec::new();
                for _ in 0..count {
                    let enemies = state.alive_monster_ids();
                    if enemies.is_empty() {
                        break;
                    }
                    let Some(index) = state.rng.combat_targets.next_usize(enemies.len()) else {
                        break;
                    };
                    effects.push(Effect::ApplyPower {
                        target: enemies[index],
                        power,
                        amount,
                        source,
                    });
                }
                ApplyResult::Continue(effects)
            }
            Effect::AddPowerAmount {
                power,
                amount,
                source,
            } => match state.add_power_amount(power, amount) {
                Ok((target, power_def, actual)) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::PowerApplied {
                            target,
                            power,
                        }));
                    ApplyResult::Continue(vec![
                        Effect::Trigger(Event::PowerApplied(PowerApplied {
                            target,
                            power: power_def,
                            instance: power,
                            amount: actual,
                            source,
                        })),
                        Effect::Trigger(Event::PowerAmountChanged(PowerAmountChanged {
                            target,
                            power: power_def,
                            instance: power,
                            delta: actual,
                            source,
                        })),
                    ])
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
            Effect::DiscardHand { player, kind } => {
                let cards = state
                    .combat()
                    .filter(|combat| combat.player.id == player)
                    .map(|combat| combat.player.piles.hand.clone())
                    .unwrap_or_default();
                if kind == DiscardKind::EndOfTurn {
                    self.cleanup_hand_at_end_turn(state, registry, player, cards)
                } else {
                    self.discard_cards(state, registry, player, cards, kind, 0)
                }
            }
            Effect::DiscardCards {
                player,
                cards,
                kind,
                then_draw,
            } => self.discard_cards(state, registry, player, cards, kind, then_draw),
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
            Effect::SelectHandCards {
                player,
                filter,
                min,
                max,
                prompt,
                source,
                on_resolve,
            } => self.select_hand_cards(
                state, registry, player, filter, min, max, prompt, source, on_resolve,
            ),
            Effect::SelectPileCards {
                player,
                pile,
                filter,
                min,
                max,
                prompt,
                source,
                on_resolve,
            } => self.select_pile_cards(
                state, registry, player, pile, filter, min, max, prompt, source, on_resolve,
            ),
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
            Effect::RetainCardsThisTurn { cards } => {
                for card in cards {
                    match state.add_card_keyword(
                        card,
                        CardKeyword::Retain,
                        CardKeywordDuration::ThisTurn,
                    ) {
                        Ok(changed) => {
                            if changed {
                                self.log.push(LogEntry::StateChanged(
                                    StateChange::CardKeywordChanged {
                                        card,
                                        keyword: CardKeyword::Retain,
                                        added: true,
                                        duration: CardKeywordDuration::ThisTurn,
                                    },
                                ));
                            }
                        }
                        Err(error) => return ApplyResult::StateError(error),
                    }
                }
                ApplyResult::Continue(Vec::new())
            }
            Effect::AddCardKeyword {
                card,
                keyword,
                duration,
                source: _,
            } => match state.add_card_keyword(card, keyword, duration) {
                Ok(changed) => {
                    if changed {
                        self.log
                            .push(LogEntry::StateChanged(StateChange::CardKeywordChanged {
                                card,
                                keyword,
                                added: true,
                                duration,
                            }));
                    }
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::RemoveCardKeyword {
                card,
                keyword,
                source: _,
            } => {
                let was_active = card_has_keyword(state, registry, card, keyword);
                match state.remove_card_keyword(card, keyword) {
                    Ok(_) => {
                        if was_active {
                            self.log.push(LogEntry::StateChanged(
                                StateChange::CardKeywordChanged {
                                    card,
                                    keyword,
                                    added: false,
                                    duration: CardKeywordDuration::Persistent,
                                },
                            ));
                        }
                        ApplyResult::Continue(Vec::new())
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::ClearCardTurnState { player } => {
                match state.clear_player_card_turn_state(player) {
                    Ok(cleared) => {
                        for (card, keywords) in cleared {
                            for keyword in keywords {
                                self.log.push(LogEntry::StateChanged(
                                    StateChange::CardKeywordChanged {
                                        card,
                                        keyword,
                                        added: false,
                                        duration: CardKeywordDuration::ThisTurn,
                                    },
                                ));
                            }
                        }
                        ApplyResult::Continue(Vec::new())
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
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
            Effect::SetPowerCounter {
                power,
                counter,
                value,
            } => match state.set_power_counter(power, counter, value) {
                Ok(value) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::PowerCounterChanged {
                            power,
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
                let candidates = random_card_candidates(registry, card_type, target);
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
            Effect::DiscoverRandomCardsToHand {
                player,
                count,
                zero_cost_this_turn,
            } => self.discover_random_cards_to_hand(
                state,
                registry,
                player,
                count,
                zero_cost_this_turn,
            ),
            Effect::PlayTopDrawCards {
                player,
                count,
                exhaust_after_play,
            } => self.apply_play_top_draw_cards(state, registry, player, count, exhaust_after_play),
            Effect::PlayRandomCardsFromPile {
                player,
                pile,
                filter,
                count,
                exhaust_after_play,
            } => self.apply_play_random_cards_from_pile(
                state,
                registry,
                player,
                pile,
                filter,
                count,
                exhaust_after_play,
            ),
            Effect::AddOrbSlots { player, amount } => {
                match state.add_orb_slots(player, amount) {
                    Ok(actual) => {
                        if actual > 0 {
                            let slots = state
                                .combat()
                                .map(|combat| combat.player.orb_queue.slots)
                                .unwrap_or_default();
                            self.log.push(LogEntry::StateChanged(
                                StateChange::OrbSlotCountChanged { player, slots },
                            ));
                        }
                        ApplyResult::Continue(Vec::new())
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::RemoveOrbSlots { player, amount } => {
                match state.remove_orb_slots(player, amount) {
                    Ok(actual) => {
                        if actual > 0 {
                            let slots = state
                                .combat()
                                .map(|combat| combat.player.orb_queue.slots)
                                .unwrap_or_default();
                            self.log.push(LogEntry::StateChanged(
                                StateChange::OrbSlotCountChanged { player, slots },
                            ));
                        }
                        ApplyResult::Continue(Vec::new())
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::ChannelOrb {
                player,
                orb,
                source,
            } => self.channel_orb(state, player, orb, source),
            Effect::ChannelRandomOrb { player, source } => {
                let pool = crate::content::orbs::RANDOM_ORB_POOL;
                let Some(index) = state.rng.combat_orbs.next_usize(pool.len()) else {
                    return ApplyResult::Continue(Vec::new());
                };
                ApplyResult::Continue(vec![Effect::ChannelOrb {
                    player,
                    orb: pool[index],
                    source,
                }])
            }
            Effect::EvokeOrb {
                player,
                target,
                remove,
                source,
            } => self.evoke_orb(state, registry, player, target, remove, source),
            Effect::TriggerOrbPassive {
                orb,
                trigger,
                target,
            } => self.trigger_orb_passive(state, registry, orb, trigger, target),
            Effect::AddOrbAmount { orb, amount } => match state.add_orb_amount(orb, amount) {
                Ok(actual) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::OrbAmountChanged {
                            orb,
                            amount: actual,
                        }));
                    ApplyResult::Continue(Vec::new())
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::SummonOsty {
                player,
                amount,
                source,
            } => self.summon_osty(state, registry, player, amount, source),
            Effect::KillCreature {
                creature,
                source: _,
            } => match state.lose_hp(
                creature,
                state
                    .creature(creature)
                    .map(|creature| Decimal::from(creature.hp))
                    .unwrap_or_default(),
            ) {
                Ok(_) => ApplyResult::Continue(vec![Effect::CheckDeaths]),
                Err(error) => ApplyResult::StateError(error),
            },
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
            Effect::ResolveChoice(resolution) => self.resolve_choice(state, registry, resolution),
        }
    }

    fn trigger_event(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        event: Event,
    ) -> ApplyResult {
        self.record_event_stats(state, registry, &event);
        self.log.push(LogEntry::EventTriggered(event.clone()));

        for listener in RulePipeline::event_listeners(state, &event) {
            let effects = RulePipeline::notify_listener(registry, state, listener, &event);
            match self.apply_immediate_effects(state, registry, effects) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
        }

        ApplyResult::Continue(Vec::new())
    }

    fn select_hand_cards(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        filter: CardFilter,
        min: usize,
        max: usize,
        prompt: LocKey,
        source: Option<Source>,
        on_resolve: ChoiceAction,
    ) -> ApplyResult {
        let cards = matching_hand_cards(state, registry, player, filter);
        if cards.is_empty() {
            return ApplyResult::Continue(Vec::new());
        }

        // C# CardSelectCmd.FromHand auto-selects all valid cards when the
        // requested count is exact and the available count is not greater.
        if min == max && cards.len() <= min {
            let effects = choice_action_effects(state, on_resolve, cards, Vec::new());
            return self.apply_immediate_effects(state, registry, effects);
        }

        let effective_max = max.min(cards.len());
        let effective_min = min.min(effective_max);
        if effective_max == 0 {
            return ApplyResult::Continue(Vec::new());
        }

        let options = cards
            .into_iter()
            .map(|card| ChoiceOption {
                id: ChoiceId::new(card.get()),
                loc_key: card_loc_key(state, registry, card),
                value: ChoiceValue::Card(card),
                enabled: true,
            })
            .collect();

        ApplyResult::NeedChoice(crate::core::effect::ChoiceRequest {
            id: ChoiceId::new(0),
            kind: ChoiceKind::SelectCard,
            source,
            prompt,
            min: effective_min,
            max: effective_max,
            on_resolve,
            options,
        })
    }

    fn select_pile_cards(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        pile: PileKind,
        filter: CardFilter,
        min: usize,
        max: usize,
        prompt: LocKey,
        source: Option<Source>,
        on_resolve: ChoiceAction,
    ) -> ApplyResult {
        let cards = matching_pile_cards(state, registry, player, pile, filter);
        if cards.is_empty() {
            return ApplyResult::Continue(Vec::new());
        }

        if min == max && cards.len() <= min {
            let effects = choice_action_effects(state, on_resolve, cards, Vec::new());
            return self.apply_immediate_effects(state, registry, effects);
        }

        let effective_max = max.min(cards.len());
        let effective_min = min.min(effective_max);
        if effective_max == 0 {
            return ApplyResult::Continue(Vec::new());
        }

        let options = cards
            .into_iter()
            .map(|card| ChoiceOption {
                id: ChoiceId::new(card.get()),
                loc_key: card_loc_key(state, registry, card),
                value: ChoiceValue::Card(card),
                enabled: true,
            })
            .collect();

        ApplyResult::NeedChoice(crate::core::effect::ChoiceRequest {
            id: ChoiceId::new(0),
            kind: ChoiceKind::SelectCard,
            source,
            prompt,
            min: effective_min,
            max: effective_max,
            on_resolve,
            options,
        })
    }

    fn resolve_choice(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        resolution: ChoiceResolution,
    ) -> ApplyResult {
        self.log.push(LogEntry::ChoiceResolved(resolution.clone()));
        let selected_cards = resolution
            .selected
            .iter()
            .filter_map(|option| match option.value {
                ChoiceValue::Card(card) => Some(card),
                ChoiceValue::CardDef(_) | ChoiceValue::Target(_) | ChoiceValue::None => None,
            })
            .collect::<Vec<_>>();
        let selected_defs = resolution
            .selected
            .iter()
            .filter_map(|option| match option.value {
                ChoiceValue::CardDef(def) => Some(def),
                ChoiceValue::Card(_) | ChoiceValue::Target(_) | ChoiceValue::None => None,
            })
            .collect::<Vec<_>>();
        let effects = choice_action_effects(
            state,
            resolution.request.on_resolve,
            selected_cards,
            selected_defs,
        );
        self.apply_immediate_effects(state, registry, effects)
    }

    fn cleanup_hand_at_end_turn(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        cards: Vec<CardInstanceId>,
    ) -> ApplyResult {
        let mut effects = Vec::new();
        let mut discard = Vec::new();

        for card in cards {
            if !state.card_is_in_pile(card, PileKind::Hand) {
                continue;
            }
            if card_has_keyword(state, registry, card, CardKeyword::Retain) {
                continue;
            }
            if card_has_keyword(state, registry, card, CardKeyword::Ethereal) {
                effects.push(Effect::ExhaustCard { card });
            } else {
                discard.push(card);
            }
        }

        for card in discard {
            match state.move_card(card, PileId::player(player, PileKind::Discard)) {
                Ok(from_kind) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::CardMoved {
                            card,
                            from: from_kind.map(|kind| PileId {
                                owner: player,
                                kind,
                            }),
                            to: PileId::player(player, PileKind::Discard),
                            reason: MoveReason::Discard,
                        }));
                    effects.push(Effect::Trigger(Event::CardDiscarded(CardDiscarded {
                        player,
                        card,
                        kind: DiscardKind::EndOfTurn,
                    })));
                }
                Err(error) => return ApplyResult::StateError(error),
            }
        }

        self.apply_immediate_effects(state, registry, effects)
    }

    fn discard_cards(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        cards: Vec<CardInstanceId>,
        kind: DiscardKind,
        then_draw: u8,
    ) -> ApplyResult {
        let mut effects = Vec::new();
        let mut sly_cards = Vec::new();
        for card in cards {
            if !state.card_is_in_pile(card, PileKind::Hand) {
                continue;
            }
            if kind == DiscardKind::Manual
                && card_has_keyword(state, registry, card, CardKeyword::Sly)
            {
                sly_cards.push(card);
            }
            match state.move_card(card, PileId::player(player, PileKind::Discard)) {
                Ok(from_kind) => {
                    self.log
                        .push(LogEntry::StateChanged(StateChange::CardMoved {
                            card,
                            from: from_kind.map(|kind| PileId {
                                owner: player,
                                kind,
                            }),
                            to: PileId::player(player, PileKind::Discard),
                            reason: MoveReason::Discard,
                        }));
                    effects.push(Effect::Trigger(Event::CardDiscarded(CardDiscarded {
                        player,
                        card,
                        kind,
                    })));
                }
                Err(error) => return ApplyResult::StateError(error),
            }
        }
        if then_draw > 0 {
            effects.push(Effect::DrawCards {
                player,
                count: then_draw,
            });
        }
        for card in sly_cards {
            effects.push(Effect::AutoPlayCard {
                player,
                card,
                target: None,
                force_exhaust: false,
                reason: AutoPlayReason::SlyDiscard,
            });
        }
        self.apply_immediate_effects(state, registry, effects)
    }

    fn channel_orb(
        &mut self,
        state: &mut GameState,
        player: PlayerId,
        orb: crate::core::ids::OrbId,
        source: Option<Source>,
    ) -> ApplyResult {
        let Some(queue) = state
            .combat()
            .filter(|combat| combat.player.id == player)
            .map(|combat| combat.player.orb_queue.clone())
        else {
            return ApplyResult::StateError(StateError::UnknownPlayer(player));
        };

        if queue.base_slots == 0 && queue.slots == 0 {
            return ApplyResult::Continue(vec![
                Effect::AddOrbSlots { player, amount: 1 },
                Effect::ChannelOrb {
                    player,
                    orb,
                    source,
                },
            ]);
        }

        if !queue.has_room() {
            return ApplyResult::Continue(vec![
                Effect::EvokeOrb {
                    player,
                    target: OrbSelection::First,
                    remove: true,
                    source,
                },
                Effect::ChannelOrb {
                    player,
                    orb,
                    source,
                },
            ]);
        }

        match state.channel_orb_with_amount(
            player,
            orb,
            crate::content::orbs::initial_orb_amount(orb),
        ) {
            Ok(instance) => {
                if orb == crate::content::orbs::LIGHTNING_ORB {
                    state.record_lightning_orb_channeled();
                }
                self.log
                    .push(LogEntry::StateChanged(StateChange::OrbChanneled {
                        orb: instance,
                    }));
                ApplyResult::Continue(vec![Effect::Trigger(Event::OrbChanneled(OrbChanneled {
                    player,
                    orb: instance,
                    orb_def: orb,
                    source,
                }))])
            }
            Err(error) => ApplyResult::StateError(error),
        }
    }

    fn evoke_orb(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        target: OrbSelection,
        remove: bool,
        source: Option<Source>,
    ) -> ApplyResult {
        let Some(orb) = select_orb(state, player, target) else {
            return ApplyResult::Continue(Vec::new());
        };
        let Some(instance) = state.orb(orb).cloned() else {
            return ApplyResult::StateError(StateError::UnknownOrb(orb));
        };
        let ctx = RuleCtx {
            state,
            registry,
            listener: Some(ListenerRef::Orb(orb)),
        };
        let effects = registry
            .orbs
            .get(instance.def)
            .map(|def| (def.evoke)(&ctx, orb, OrbTrigger::BeforeTurnEnd, None))
            .unwrap_or_default();
        match self.apply_immediate_effects(state, registry, effects) {
            ApplyResult::Continue(_) => {}
            other => return other,
        }

        if remove {
            match state.remove_orb(orb) {
                Ok(_) => {}
                Err(error) => return ApplyResult::StateError(error),
            }
        }
        self.log
            .push(LogEntry::StateChanged(StateChange::OrbEvoked {
                orb,
                removed: remove,
            }));
        ApplyResult::Continue(vec![Effect::Trigger(Event::OrbEvoked(OrbEvoked {
            player,
            orb,
            orb_def: instance.def,
            removed: remove,
            source,
            targets: Vec::new(),
        }))])
    }

    fn trigger_orb_passive(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        orb: OrbInstanceId,
        trigger: OrbTrigger,
        target: Option<CreatureId>,
    ) -> ApplyResult {
        let Some(instance) = state.orb(orb).cloned() else {
            return ApplyResult::Continue(Vec::new());
        };
        let calc = OrbPassiveTriggerCountCalc {
            player: instance.owner,
            orb,
            trigger,
            base_count: 1,
            count: 1,
        };
        let (calc, modifiers) =
            RulePipeline::modify_orb_passive_trigger_count(registry, state, calc);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
        if calc.count <= 0 {
            return ApplyResult::Continue(Vec::new());
        }

        for _ in 0..calc.count {
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(ListenerRef::Orb(orb)),
            };
            let effects = registry
                .orbs
                .get(instance.def)
                .map(|def| (def.passive)(&ctx, orb, trigger, target))
                .unwrap_or_default();
            match self.apply_immediate_effects(state, registry, effects) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
        }
        ApplyResult::Continue(Vec::new())
    }

    fn summon_osty(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        amount: Decimal,
        source: Option<Source>,
    ) -> ApplyResult {
        let calc = SummonAmountCalc {
            player,
            source,
            base_amount: amount,
            amount,
        };
        let (calc, modifiers) = RulePipeline::modify_summon_amount(registry, state, calc);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
        let amount = decimal_to_i32_trunc(calc.amount).max(0);
        if amount == 0 {
            return ApplyResult::Continue(Vec::new());
        }

        let Some(combat) = state.combat_mut() else {
            return ApplyResult::StateError(StateError::CombatNotActive);
        };
        if combat.player.id != player {
            return ApplyResult::StateError(StateError::UnknownPlayer(player));
        }

        let existing = combat.creatures.iter().position(|creature| {
            creature.pet_owner == Some(player) && creature.pet_kind == Some(PlayerPetKind::Osty)
        });
        let creature_id = if let Some(index) = existing {
            let creature = &mut combat.creatures[index];
            if creature.alive {
                creature.max_hp = creature.max_hp.saturating_add(amount);
                creature.hp = creature.hp.saturating_add(amount).min(creature.max_hp);
            } else {
                creature.alive = true;
                creature.max_hp = amount;
                creature.hp = amount;
            }
            creature.id
        } else {
            let next = combat
                .creatures
                .iter()
                .map(|creature| creature.id.get())
                .max()
                .unwrap_or(1)
                + 1;
            let id = CreatureId::new(next);
            combat.creatures.push(
                crate::core::state::Creature::new(id, Side::Player, amount)
                    .with_pet(player, PlayerPetKind::Osty),
            );
            id
        };

        let mut effects = vec![Effect::Trigger(Event::Summoned(Summoned {
            player,
            creature: creature_id,
            amount,
            source,
        }))];
        if let Some(owner) = state.player_creature_id() {
            effects.push(Effect::ApplyPower {
                target: owner,
                power: crate::content::powers::DIE_FOR_YOU_POWER,
                amount: Decimal::from(1),
                source,
            });
        }
        ApplyResult::Continue(effects)
    }

    fn apply_damage(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        op: DamageOp,
    ) -> ApplyResult {
        let target = op.target;
        let Some(creature) = state.creature(target) else {
            return ApplyResult::StateError(StateError::UnknownCreature(target));
        };
        if !creature.is_hittable() {
            return ApplyResult::Continue(Vec::new());
        }

        let calc = DamageCalc {
            source: op.source,
            dealer: op.dealer,
            target,
            kind: op.kind,
            base_amount: op.base_amount,
            amount: op.base_amount,
        };
        let (calc, modifiers) = RulePipeline::modify_damage(registry, state, calc);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));

        let requested = if calc.amount < Decimal::from(0) {
            Decimal::from(0)
        } else {
            calc.amount
        };

        let original_target = target;
        let block_before = creature.block;
        let mut blocked = 0;
        let mut hp_loss = requested;

        if !op.flags.ignores_block {
            let blocked_decimal = if Decimal::from(block_before) < requested {
                Decimal::from(block_before)
            } else {
                requested
            };
            let block_loss = decimal_to_i32_trunc(blocked_decimal);
            blocked = match state.lose_block(target, block_loss) {
                Ok(actual) => actual,
                Err(error) => return ApplyResult::StateError(error),
            };
            hp_loss = requested - blocked_decimal;
        }

        let before_redirect = HpLossCalc {
            source: op.source,
            dealer: op.dealer,
            target: original_target,
            kind: op.kind,
            base_amount: hp_loss,
            amount: hp_loss,
            phase: HpLossPhase::BeforeRedirect,
        };
        let (before_redirect, modifiers) =
            RulePipeline::modify_hp_loss(registry, state, before_redirect);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
        hp_loss = before_redirect.amount.max(Decimal::from(0));

        let redirect = UnblockedDamageTargetCalc {
            source: op.source,
            dealer: op.dealer,
            original_target,
            target: original_target,
            amount: hp_loss,
        };
        let (redirect, modifiers) =
            RulePipeline::modify_unblocked_damage_target(registry, state, redirect);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
        let damage_target = redirect.target;

        let after_redirect = HpLossCalc {
            source: op.source,
            dealer: op.dealer,
            target: damage_target,
            kind: op.kind,
            base_amount: hp_loss,
            amount: hp_loss,
            phase: HpLossPhase::AfterRedirect,
        };
        let (after_redirect, modifiers) =
            RulePipeline::modify_hp_loss(registry, state, after_redirect);
        self.log
            .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
        hp_loss = after_redirect.amount.max(Decimal::from(0));

        let hp_before = state
            .creature(damage_target)
            .map(|creature| creature.hp)
            .unwrap_or(0);
        let actual_hp_loss = match state.lose_hp(damage_target, hp_loss) {
            Ok(actual) => actual,
            Err(error) => return ApplyResult::StateError(error),
        };
        let hp_after = hp_before - actual_hp_loss;

        let result = DamageResult {
            source: op.source,
            dealer: op.dealer,
            target: damage_target,
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
                creature: damage_target,
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

    fn finish_card_play(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
        force_exhaust: bool,
    ) -> ApplyResult {
        match self.apply_immediate_effects(
            state,
            registry,
            vec![Effect::Trigger(Event::CardPlayed(CardPlayed {
                player,
                card,
                target,
            }))],
        ) {
            ApplyResult::Continue(_) => {}
            other => return other,
        }

        if !state.card_is_in_pile(card, PileKind::Play) {
            self.pending_card_results.remove(&card);
            return ApplyResult::Continue(Vec::new());
        }

        let result = if let Some(result) = self.pending_card_results.remove(&card) {
            result
        } else {
            let (pile, reason, modifiers) =
                match resolve_card_play_result_pile(state, registry, player, card, force_exhaust) {
                    Ok(result) => result,
                    Err(error) => return ApplyResult::StateError(error),
                };
            self.log.extend(
                modifiers
                    .into_iter()
                    .map(LogEntry::CardPlayResultPileModified),
            );
            CardPlayResult { pile, reason }
        };

        match result.reason {
            MoveReason::Exhaust => {
                self.apply_immediate_effects(state, registry, vec![Effect::ExhaustCard { card }])
            }
            _ => self.apply_immediate_effects(
                state,
                registry,
                vec![Effect::MoveCard {
                    card,
                    to: result.pile,
                    reason: result.reason,
                }],
            ),
        }
    }

    fn prepare_card_play_result(
        &mut self,
        state: &GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        card: CardInstanceId,
        force_exhaust: bool,
    ) -> ApplyResult {
        let (pile, reason, modifiers) =
            match resolve_card_play_result_pile(state, registry, player, card, force_exhaust) {
                Ok(result) => result,
                Err(error) => return ApplyResult::StateError(error),
            };
        self.pending_card_results
            .insert(card, CardPlayResult { pile, reason });
        self.log.extend(
            modifiers
                .into_iter()
                .map(LogEntry::CardPlayResultPileModified),
        );
        ApplyResult::Continue(Vec::new())
    }

    fn record_event_stats(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        event: &Event,
    ) {
        if let Event::CardPlayStarted(event) = event {
            state.record_card_played();
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

        for card in cards {
            match self.auto_play_card(state, registry, player, card, None, exhaust_after_play) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
        }
        ApplyResult::Continue(Vec::new())
    }

    fn apply_play_random_cards_from_pile(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: crate::core::ids::PlayerId,
        pile: PileKind,
        filter: CardFilter,
        count: u8,
        exhaust_after_play: bool,
    ) -> ApplyResult {
        let mut cards = matching_pile_cards(state, registry, player, pile, filter);
        state.rng.shuffle.shuffle(&mut cards);
        cards.truncate(count as usize);

        for card in cards {
            match self.auto_play_card(state, registry, player, card, None, exhaust_after_play) {
                ApplyResult::Continue(_) => {}
                other => return other,
            }
            if combat_result_for_state(state).is_some() {
                break;
            }
        }
        ApplyResult::Continue(Vec::new())
    }

    fn auto_play_card(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
        force_exhaust: bool,
    ) -> ApplyResult {
        if !state
            .combat()
            .map(|combat| combat.cards.contains_key(&card))
            .unwrap_or(false)
        {
            return ApplyResult::Continue(Vec::new());
        }

        let target = target.or_else(|| auto_play_target(state, registry, card));
        if card_has_keyword(state, registry, card, CardKeyword::Unplayable)
            || validate_card_target(state, registry, card, target).is_err()
        {
            return self.move_card_to_result_without_playing(
                state,
                registry,
                player,
                card,
                force_exhaust,
            );
        }

        let decision = RulePipeline::should_play(registry, state, card, target);
        self.log.push(LogEntry::DecisionMade(decision.clone()));
        if !decision.is_allowed() {
            return self.move_card_to_result_without_playing(
                state,
                registry,
                player,
                card,
                force_exhaust,
            );
        }

        self.apply_immediate_effects(
            state,
            registry,
            auto_play_card_effects(player, card, target, force_exhaust),
        )
    }

    fn move_card_to_result_without_playing(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        player: PlayerId,
        card: CardInstanceId,
        force_exhaust: bool,
    ) -> ApplyResult {
        let (pile, reason, modifiers) =
            match resolve_card_play_result_pile(state, registry, player, card, force_exhaust) {
                Ok(result) => result,
                Err(error) => return ApplyResult::StateError(error),
            };
        self.log.extend(
            modifiers
                .into_iter()
                .map(LogEntry::CardPlayResultPileModified),
        );

        let mut effects = vec![Effect::MoveCard {
            card,
            to: PileId::player(player, PileKind::Play),
            reason: MoveReason::Play,
        }];
        match reason {
            MoveReason::Exhaust => effects.push(Effect::ExhaustCard { card }),
            _ => effects.push(Effect::MoveCard {
                card,
                to: pile,
                reason,
            }),
        }
        self.apply_immediate_effects(state, registry, effects)
    }

    fn discover_random_cards_to_hand(
        &mut self,
        state: &mut GameState,
        registry: &StaticRegistry,
        _player: PlayerId,
        count: u8,
        zero_cost_this_turn: bool,
    ) -> ApplyResult {
        let mut candidates = random_card_candidates(registry, None, None);
        let mut defs = Vec::new();
        for _ in 0..count {
            let Some(index) = state
                .rng
                .combat_card_generation
                .next_usize(candidates.len())
            else {
                break;
            };
            defs.push(candidates.swap_remove(index));
        }

        if defs.is_empty() {
            return ApplyResult::Continue(Vec::new());
        }

        let options = defs
            .into_iter()
            .enumerate()
            .map(|(index, def)| ChoiceOption {
                id: ChoiceId::new((index + 1) as u32),
                loc_key: registry
                    .cards
                    .get(def)
                    .map(|def| def.loc_key)
                    .unwrap_or_else(|| LocKey::new("card.UNKNOWN")),
                value: ChoiceValue::CardDef(def),
                enabled: true,
            })
            .collect();

        ApplyResult::NeedChoice(crate::core::effect::ChoiceRequest {
            id: ChoiceId::new(0),
            kind: ChoiceKind::SelectCard,
            source: None,
            prompt: LocKey::new("choice.discover_card"),
            min: 0,
            max: 1,
            on_resolve: ChoiceAction::AddSelectedCardDefsToHand {
                upgraded: false,
                temporary: true,
                zero_cost_this_turn,
            },
            options,
        })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CardPlayResult {
    pile: PileId,
    reason: MoveReason,
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
        .filter(|card| card_matches_filter(state, registry, *card, filter))
        .collect()
}

fn matching_pile_cards(
    state: &GameState,
    registry: &StaticRegistry,
    player: crate::core::ids::PlayerId,
    pile: PileKind,
    filter: CardFilter,
) -> Vec<crate::core::ids::CardInstanceId> {
    let Some(combat) = state.combat().filter(|combat| combat.player.id == player) else {
        return Vec::new();
    };
    combat
        .player
        .piles
        .pile(pile)
        .iter()
        .copied()
        .filter(|card| card_matches_filter(state, registry, *card, filter))
        .collect()
}

fn card_matches_filter(
    state: &GameState,
    registry: &StaticRegistry,
    card: CardInstanceId,
    filter: CardFilter,
) -> bool {
    match filter {
        CardFilter::Any => true,
        CardFilter::Attack => state
            .card(card)
            .and_then(|card| registry.cards.get(card.def))
            .map(|def| def.card_type == CardType::Attack)
            .unwrap_or(false),
        CardFilter::NonAttack => state
            .card(card)
            .and_then(|card| registry.cards.get(card.def))
            .map(|def| def.card_type != CardType::Attack)
            .unwrap_or(false),
        CardFilter::SkillWithoutKeyword(keyword) => {
            state
                .card(card)
                .and_then(|card| registry.cards.get(card.def))
                .map(|def| def.card_type == CardType::Skill)
                .unwrap_or(false)
                && !card_has_keyword(state, registry, card, keyword)
        }
        CardFilter::NotRetainedThisTurn => {
            !card_has_keyword(state, registry, card, CardKeyword::Retain)
        }
    }
}

fn random_card_candidates(
    registry: &StaticRegistry,
    card_type: Option<CardType>,
    target: Option<TargetType>,
) -> Vec<CardId> {
    registry
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
        .collect()
}

fn auto_play_card_effects(
    player: PlayerId,
    card: CardInstanceId,
    target: Option<CreatureId>,
    exhaust_after_play: bool,
) -> Vec<Effect> {
    let play = PileId::player(player, PileKind::Play);
    vec![
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
        Effect::PrepareCardPlayResult {
            player,
            card,
            force_exhaust: exhaust_after_play,
        },
        Effect::ExecuteCardBody {
            player,
            card,
            target,
        },
        Effect::FinishCardPlay {
            player,
            card,
            target,
            force_exhaust: exhaust_after_play,
        },
    ]
}

fn auto_play_target(
    state: &mut GameState,
    registry: &StaticRegistry,
    card: CardInstanceId,
) -> Option<CreatureId> {
    let target_type = state
        .card(card)
        .and_then(|card| registry.cards.get(card.def))
        .map(|def| def.target)?;
    match target_type {
        TargetType::Enemy | TargetType::AnyCreature => {
            let enemies = state.alive_monster_ids();
            state
                .rng
                .combat_targets
                .next_usize(enemies.len())
                .map(|index| enemies[index])
        }
        _ => None,
    }
}

fn source_creature(state: &GameState, source: Option<Source>) -> Option<CreatureId> {
    match source {
        Some(Source::Creature(creature)) => Some(creature),
        Some(Source::Card(card)) => state
            .card(card)
            .and_then(|card| state.combat().map(|combat| (card.owner, combat)))
            .and_then(|(owner, combat)| {
                (combat.player.id == owner).then_some(combat.player.creature)
            }),
        Some(Source::Power(power)) => state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
            .map(|power| power.owner),
        Some(Source::Relic(_)) | Some(Source::Potion(_)) | Some(Source::System) | None => None,
    }
}

fn select_orb(
    state: &GameState,
    player: PlayerId,
    selection: OrbSelection,
) -> Option<OrbInstanceId> {
    let queue = state
        .combat()
        .filter(|combat| combat.player.id == player)
        .map(|combat| &combat.player.orb_queue)?;
    match selection {
        OrbSelection::First => queue.orbs.first().copied(),
        OrbSelection::Last => queue.orbs.last().copied(),
        OrbSelection::Exact(orb) => queue.orbs.contains(&orb).then_some(orb),
    }
}

fn card_loc_key(
    state: &GameState,
    registry: &StaticRegistry,
    card: crate::core::ids::CardInstanceId,
) -> LocKey {
    state
        .card(card)
        .and_then(|card| registry.cards.get(card.def))
        .map(|def| def.loc_key)
        .unwrap_or_else(|| LocKey::new("card.UNKNOWN"))
}

fn choice_action_effects(
    state: &GameState,
    action: ChoiceAction,
    cards: Vec<CardInstanceId>,
    defs: Vec<CardId>,
) -> Vec<Effect> {
    match action {
        ChoiceAction::None => Vec::new(),
        ChoiceAction::ExhaustSelectedCards => cards
            .into_iter()
            .map(|card| Effect::ExhaustCard { card })
            .collect(),
        ChoiceAction::DiscardSelectedCards => selected_discard_effect(state, cards, 0),
        ChoiceAction::DiscardSelectedCardsThenDraw(count) => {
            selected_discard_effect(state, cards, count)
        }
        ChoiceAction::DiscardSelectedCardsThenAddCard {
            def,
            count,
            upgraded,
        } => selected_discard_then_add_card_effects(state, cards, def, count, upgraded),
        ChoiceAction::MoveSelectedCardsToPile { pile, reason } => {
            selected_move_effects(state, cards, pile, reason)
        }
        ChoiceAction::RetainSelectedCardsThisTurn => {
            vec![Effect::RetainCardsThisTurn { cards }]
        }
        ChoiceAction::AddSelectedCardKeyword { keyword, duration } => cards
            .into_iter()
            .map(|card| Effect::AddCardKeyword {
                card,
                keyword,
                duration,
                source: None,
            })
            .collect(),
        ChoiceAction::AddSelectedCardDefsToHand {
            upgraded,
            temporary,
            zero_cost_this_turn,
        } => {
            selected_add_defs_to_hand_effects(state, defs, upgraded, temporary, zero_cost_this_turn)
        }
    }
}

fn selected_discard_effect(
    state: &GameState,
    cards: Vec<CardInstanceId>,
    then_draw: u8,
) -> Vec<Effect> {
    if cards.is_empty() {
        return Vec::new();
    }
    let Some(player) = cards
        .first()
        .and_then(|card| state.card(*card))
        .map(|card| card.owner)
        .or_else(|| state.player_id())
    else {
        return Vec::new();
    };
    vec![Effect::DiscardCards {
        player,
        cards,
        kind: DiscardKind::Manual,
        then_draw,
    }]
}

fn selected_discard_then_add_card_effects(
    state: &GameState,
    cards: Vec<CardInstanceId>,
    def: crate::core::ids::CardId,
    count: u8,
    upgraded: bool,
) -> Vec<Effect> {
    let Some(player) = cards
        .first()
        .and_then(|card| state.card(*card))
        .map(|card| card.owner)
        .or_else(|| state.player_id())
    else {
        return Vec::new();
    };
    let mut effects = selected_discard_effect(state, cards, 0);
    for _ in 0..count {
        effects.push(Effect::AddGeneratedCard {
            player,
            def,
            to: PileId::player(player, PileKind::Hand),
            upgraded,
            temporary: true,
            zero_cost_this_turn: false,
        });
    }
    effects
}

fn selected_move_effects(
    state: &GameState,
    cards: Vec<CardInstanceId>,
    pile: PileKind,
    reason: MoveReason,
) -> Vec<Effect> {
    let Some(player) = cards
        .first()
        .and_then(|card| state.card(*card))
        .map(|card| card.owner)
        .or_else(|| state.player_id())
    else {
        return Vec::new();
    };
    cards
        .into_iter()
        .map(|card| Effect::MoveCard {
            card,
            to: PileId::player(player, pile),
            reason,
        })
        .collect()
}

fn selected_add_defs_to_hand_effects(
    state: &GameState,
    defs: Vec<CardId>,
    upgraded: bool,
    temporary: bool,
    zero_cost_this_turn: bool,
) -> Vec<Effect> {
    let Some(player) = state.player_id() else {
        return Vec::new();
    };
    defs.into_iter()
        .map(|def| Effect::AddGeneratedCard {
            player,
            def,
            to: PileId::player(player, PileKind::Hand),
            upgraded,
            temporary,
            zero_cost_this_turn,
        })
        .collect()
}

fn card_has_keyword(
    state: &GameState,
    registry: &StaticRegistry,
    card: CardInstanceId,
    keyword: CardKeyword,
) -> bool {
    state.card_has_keyword(registry, card, keyword)
}

fn resolve_card_play_result_pile(
    state: &GameState,
    registry: &StaticRegistry,
    player: PlayerId,
    card: CardInstanceId,
    force_exhaust: bool,
) -> Result<(PileId, MoveReason, Vec<CardPlayResultPileModifierLog>), StateError> {
    let base_pile = base_card_play_result_pile(state, registry, player, card, force_exhaust)?;
    let calc = CardPlayResultPileCalc {
        card,
        base_pile,
        pile: base_pile,
        position: PilePosition::Bottom,
    };
    let (calc, modifiers) = RulePipeline::modify_card_play_result_pile(registry, state, calc);
    let reason = move_reason_for_result_pile(calc.pile.kind);
    Ok((calc.pile, reason, modifiers))
}

fn base_card_play_result_pile(
    state: &GameState,
    registry: &StaticRegistry,
    player: PlayerId,
    card: CardInstanceId,
    force_exhaust: bool,
) -> Result<PileId, StateError> {
    let card_state = state.card(card).ok_or(StateError::UnknownCard(card))?;
    let Some(def) = registry.cards.get(card_state.def) else {
        return Ok(PileId::player(player, PileKind::Discard));
    };
    if def.card_type == CardType::Power
        || card_has_keyword(state, registry, card, CardKeyword::PurgeOnUse)
    {
        Ok(PileId::player(player, PileKind::Removed))
    } else if force_exhaust || card_has_keyword(state, registry, card, CardKeyword::Exhaust) {
        Ok(PileId::player(player, PileKind::Exhaust))
    } else {
        Ok(PileId::player(player, PileKind::Discard))
    }
}

fn move_reason_for_result_pile(kind: PileKind) -> MoveReason {
    match kind {
        PileKind::Discard => MoveReason::Discard,
        PileKind::Exhaust => MoveReason::Exhaust,
        PileKind::Removed => MoveReason::Removed,
        PileKind::Draw | PileKind::Hand | PileKind::Limbo | PileKind::Play => MoveReason::Cleanup,
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
                Some(creature) if creature.side == Side::Monsters && creature.is_hittable() => {
                    Ok(())
                }
                _ => Err(PreventReason::NoValidTarget),
            }
        }
        TargetType::AllEnemies | TargetType::RandomEnemy | TargetType::AllAllies => Ok(()),
        TargetType::SelfTarget | TargetType::AnyPlayer => {
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
                Some(creature) if creature.side == Side::Player && creature.is_hittable() => Ok(()),
                _ => Err(PreventReason::NoValidTarget),
            }
        }
        TargetType::AnyCreature => {
            let Some(target) = target else {
                return Err(PreventReason::NoValidTarget);
            };
            match state.creature(target) {
                Some(creature) if creature.is_hittable() => Ok(()),
                _ => Err(PreventReason::NoValidTarget),
            }
        }
        TargetType::TargetedNoCreature | TargetType::Osty => {
            if target.is_none() {
                Ok(())
            } else {
                Err(PreventReason::NoValidTarget)
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
                for orb in combat.player.orb_queue.orbs.iter().copied() {
                    effects.push(Effect::TriggerOrbPassive {
                        orb,
                        trigger: OrbTrigger::AfterTurnStart,
                        target: None,
                    });
                }
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
                effects.push(Effect::Trigger(Event::BeforeHandDraw { player }));
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
                let mut effects = Vec::new();
                if let Some(combat) = state.combat() {
                    for orb in combat.player.orb_queue.orbs.iter().copied() {
                        effects.push(Effect::TriggerOrbPassive {
                            orb,
                            trigger: OrbTrigger::BeforeTurnEnd,
                            target: None,
                        });
                    }
                }
                effects.extend([
                    Effect::DiscardHand {
                        player,
                        kind: DiscardKind::EndOfTurn,
                    },
                    Effect::ClearCardTurnState { player },
                    Effect::CheckCombatEnd,
                    Effect::StartTurn(Side::Monsters),
                ]);
                effects
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
        CardDef, CardKeyword, CardPlayCtx, CardPoolId, CardRarity, CardRules, CardType, TargetType,
    };
    use crate::content::orbs::{OrbDef, OrbRules};
    use crate::content::powers::{PowerDef, PowerRules, CALAMITY_POWER, DOOM_POWER, POISON_POWER};
    use crate::content::relics::{RelicDef, RelicRules};
    use crate::core::effect::{
        CardFilter, ChoiceAction, ChoiceResponse, ChoiceValue, DamageAllEnemiesOp, DamageFlags,
        DamageKind, DamageOp, DiscardKind, Effect, MoveReason, OrbTrigger, Source,
    };
    use crate::core::engine::{CombatOutcome, Engine, StepResult};
    use crate::core::event::Event;
    use crate::core::ids::{
        CardId, CardInstanceId, CreatureId, LocKey, OrbId, OrbInstanceId, PowerId, PowerInstanceId,
        RelicId, RelicInstanceId,
    };
    use crate::core::log::{LogEntry, StateChange};
    use crate::core::query::{
        DamageCalc, Decision, DecisionQuery, DecisionQueryKind, OrbPassiveTriggerCountCalc,
        PowerAmountCalc, PowerAmountPhase, PreventReason, SummonAmountCalc,
        UnblockedDamageTargetCalc,
    };
    use crate::core::rules::{prevent_by_current_listener, RuleCtx};
    use crate::core::state::{
        CardCost, CardCosts, CardKeywordDuration, CombatSetupCard, CombatSetupMonster, GameState,
        PileId, PileKind, PlayerPetKind, RelicInstance, ResourceKind,
    };
    use crate::core::Command;
    use crate::registry::StaticRegistry;

    use super::EffectResolver;

    const STARTER_STRIKE: CardId = CardId::new("starter_strike");
    const TEST_STRENGTH: PowerId = PowerId::new("test_strength");
    const TEST_CANNOT_DIE: PowerId = PowerId::new("test_cannot_die");
    const TEST_OSTY_REDIRECT: PowerId = PowerId::new("test_osty_redirect");
    const TEST_MARK_ON_CARD_PLAY: PowerId = PowerId::new("test_mark_on_card_play");
    const TEST_OBSERVE_ON_CARD_PLAY: PowerId = PowerId::new("test_observe_on_card_play");
    const TEST_RELIC: RelicId = RelicId::new("test_relic");
    const TEST_ORB: OrbId = OrbId::new("test_orb");
    const TEST_SLY_SKILL: CardId = CardId::new("test_sly_skill");
    const TEST_BLANK_SKILL: CardId = CardId::new("test_blank_skill");

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
            },
        })]
    }

    fn test_strike_def() -> CardDef {
        CardDef {
            id: STARTER_STRIKE,
            loc_key: LocKey::new("card.starter_strike"),
            pool: CardPoolId::Ironclad,
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

    fn test_gain_energy_body(
        ctx: &CardPlayCtx<'_>,
        card: CardInstanceId,
        _: Option<CreatureId>,
    ) -> Vec<Effect> {
        ctx.state
            .card(card)
            .map(|card_state| {
                vec![Effect::GainResource {
                    player: card_state.owner,
                    resource: ResourceKind::Energy,
                    amount: 1,
                }]
            })
            .unwrap_or_default()
    }

    fn test_no_effect_body(
        _: &CardPlayCtx<'_>,
        _: CardInstanceId,
        _: Option<CreatureId>,
    ) -> Vec<Effect> {
        Vec::new()
    }

    fn test_skill_def(id: CardId, play: crate::content::cards::CardPlayFn) -> CardDef {
        CardDef {
            id,
            loc_key: LocKey::new("card.test_skill"),
            pool: CardPoolId::Silent,
            card_type: CardType::Skill,
            rarity: CardRarity::Common,
            target: TargetType::SelfTarget,
            base_costs: CardCosts::energy(3),
            upgraded_costs: None,
            keywords: &[],
            upgraded_keywords: &[],
            tags: &[],
            can_generate_in_combat: true,
            play,
            rules: CardRules::default(),
        }
    }

    #[test]
    fn runtime_card_keywords_support_persistent_and_turn_limited_state() {
        let mut registry = StaticRegistry::empty();
        registry
            .cards
            .register(test_skill_def(TEST_BLANK_SKILL, test_no_effect_body));
        let mut state = GameState::single_player_test_combat(
            301,
            [CombatSetupCard {
                def: TEST_BLANK_SKILL,
                upgraded: false,
                costs: CardCosts::energy(3),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            1,
        );
        let player = state.player_id().unwrap();
        let card = state.combat().unwrap().player.piles.hand[0];
        let mut resolver = EffectResolver::default();

        resolver.enqueue(Effect::AddCardKeyword {
            card,
            keyword: CardKeyword::Sly,
            duration: CardKeywordDuration::Persistent,
            source: None,
        });
        let StepResult::Done(log) = resolver.drain(&mut state, &registry) else {
            panic!("expected persistent keyword add to finish");
        };
        assert!(state.card_has_keyword(&registry, card, CardKeyword::Sly));
        assert!(log.iter().any(|entry| {
            matches!(
                entry,
                LogEntry::StateChanged(StateChange::CardKeywordChanged {
                    card: changed,
                    keyword: CardKeyword::Sly,
                    added: true,
                    duration: CardKeywordDuration::Persistent,
                }) if *changed == card
            )
        }));

        resolver.enqueue(Effect::RemoveCardKeyword {
            card,
            keyword: CardKeyword::Sly,
            source: None,
        });
        let StepResult::Done(_) = resolver.drain(&mut state, &registry) else {
            panic!("expected persistent keyword remove to finish");
        };
        assert!(!state.card_has_keyword(&registry, card, CardKeyword::Sly));

        resolver.enqueue(Effect::AddCardKeyword {
            card,
            keyword: CardKeyword::Retain,
            duration: CardKeywordDuration::ThisTurn,
            source: None,
        });
        let StepResult::Done(_) = resolver.drain(&mut state, &registry) else {
            panic!("expected turn keyword add to finish");
        };
        assert!(state.card_has_keyword(&registry, card, CardKeyword::Retain));

        resolver.enqueue(Effect::RemoveCardKeyword {
            card,
            keyword: CardKeyword::Retain,
            source: None,
        });
        let StepResult::Done(_) = resolver.drain(&mut state, &registry) else {
            panic!("expected persistent keyword remove to finish");
        };
        assert!(
            state.card_has_keyword(&registry, card, CardKeyword::Retain),
            "persistent removal must not clear a keyword granted only for this turn"
        );

        resolver.enqueue(Effect::ClearCardTurnState { player });
        let StepResult::Done(log) = resolver.drain(&mut state, &registry) else {
            panic!("expected turn keyword cleanup to finish");
        };
        assert!(!state.card_has_keyword(&registry, card, CardKeyword::Retain));
        assert!(log.iter().any(|entry| {
            matches!(
                entry,
                LogEntry::StateChanged(StateChange::CardKeywordChanged {
                    card: changed,
                    keyword: CardKeyword::Retain,
                    added: false,
                    duration: CardKeywordDuration::ThisTurn,
                }) if *changed == card
            )
        }));
    }

    #[test]
    fn manual_discard_autoplays_runtime_sly_after_discard_and_then_draw() {
        let mut registry = StaticRegistry::empty();
        registry
            .cards
            .register(test_skill_def(TEST_SLY_SKILL, test_gain_energy_body));
        registry
            .cards
            .register(test_skill_def(TEST_BLANK_SKILL, test_no_effect_body));
        let mut state = GameState::single_player_test_combat(
            302,
            [CombatSetupCard {
                def: TEST_SLY_SKILL,
                upgraded: false,
                costs: CardCosts::energy(3),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            0,
            1,
        );
        let player = state.player_id().unwrap();
        let sly_card = state.combat().unwrap().player.piles.hand[0];
        let drawn_card = state
            .add_generated_card(
                player,
                TEST_BLANK_SKILL,
                PileId::player(player, PileKind::Draw),
                false,
                CardCosts::energy(3),
                false,
                false,
            )
            .unwrap();
        let mut resolver = EffectResolver::default();

        resolver.enqueue(Effect::AddCardKeyword {
            card: sly_card,
            keyword: CardKeyword::Sly,
            duration: CardKeywordDuration::ThisTurn,
            source: None,
        });
        let StepResult::Done(_) = resolver.drain(&mut state, &registry) else {
            panic!("expected runtime sly add to finish");
        };

        resolver.enqueue(Effect::DiscardCards {
            player,
            cards: vec![sly_card],
            kind: DiscardKind::Manual,
            then_draw: 1,
        });
        let StepResult::Done(log) = resolver.drain(&mut state, &registry) else {
            panic!("expected manual discard to finish");
        };

        assert_eq!(state.combat().unwrap().player.energy, 1);
        assert!(state.card_is_in_pile(sly_card, PileKind::Discard));
        assert!(state.card_is_in_pile(drawn_card, PileKind::Hand));

        let discard_pos = log
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    LogEntry::EventTriggered(Event::CardDiscarded(event))
                        if event.card == sly_card && event.kind == DiscardKind::Manual
                )
            })
            .expect("manual discard event should be logged");
        let draw_pos = log
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    LogEntry::EventTriggered(Event::CardDrawn(event))
                        if event.card == drawn_card
                )
            })
            .expect("then_draw should draw before Sly autoplay");
        let play_pos = log
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    LogEntry::EventTriggered(Event::CardPlayed(event))
                        if event.card == sly_card
                )
            })
            .expect("Sly discard should autoplay the discarded card");

        assert!(discard_pos < draw_pos);
        assert!(draw_pos < play_pos);
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

    fn mark_on_card_play(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
        if !matches!(event, Event::CardPlayed(_)) {
            return Vec::new();
        }
        ctx.state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
            .map(|instance| {
                vec![Effect::ApplyPower {
                    target: instance.owner,
                    power: TEST_STRENGTH,
                    amount: Decimal::from(1),
                    source: Some(Source::Power(power)),
                }]
            })
            .unwrap_or_default()
    }

    fn observe_marker_on_card_play(
        ctx: &RuleCtx<'_>,
        power: PowerInstanceId,
        event: &Event,
    ) -> Vec<Effect> {
        if !matches!(event, Event::CardPlayed(_)) {
            return Vec::new();
        }
        let Some(owner) = ctx
            .state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
            .map(|instance| instance.owner)
        else {
            return Vec::new();
        };
        if ctx.state.power_amount(owner, TEST_STRENGTH) <= 0 {
            return Vec::new();
        }
        vec![Effect::GainBlock {
            target: owner,
            amount: Decimal::from(1),
            source: Some(Source::Power(power)),
        }]
    }

    fn mark_on_card_play_def() -> PowerDef {
        PowerDef {
            id: TEST_MARK_ON_CARD_PLAY,
            loc_key: LocKey::new("power.test_mark_on_card_play"),
            rules: PowerRules {
                on_event: Some(mark_on_card_play),
                ..PowerRules::default()
            },
        }
    }

    fn observe_on_card_play_def() -> PowerDef {
        PowerDef {
            id: TEST_OBSERVE_ON_CARD_PLAY,
            loc_key: LocKey::new("power.test_observe_on_card_play"),
            rules: PowerRules {
                on_event: Some(observe_marker_on_card_play),
                ..PowerRules::default()
            },
        }
    }

    fn poison_amount_relic(
        _ctx: &RuleCtx<'_>,
        _relic: RelicInstanceId,
        mut calc: PowerAmountCalc,
    ) -> PowerAmountCalc {
        if calc.power == POISON_POWER && calc.phase == PowerAmountPhase::Given {
            calc.amount += Decimal::from(1);
        }
        calc
    }

    fn orb_trigger_relic(
        _ctx: &RuleCtx<'_>,
        _relic: RelicInstanceId,
        mut calc: OrbPassiveTriggerCountCalc,
    ) -> OrbPassiveTriggerCountCalc {
        calc.count += 1;
        calc
    }

    fn summon_amount_relic(
        _ctx: &RuleCtx<'_>,
        _relic: RelicInstanceId,
        mut calc: SummonAmountCalc,
    ) -> SummonAmountCalc {
        calc.amount += Decimal::from(2);
        calc
    }

    fn test_relic() -> RelicDef {
        RelicDef {
            id: TEST_RELIC,
            loc_key: LocKey::new("relic.test"),
            rules: RelicRules {
                modify_power_amount: Some(poison_amount_relic),
                modify_orb_passive_trigger_count: Some(orb_trigger_relic),
                modify_summon_amount: Some(summon_amount_relic),
                ..RelicRules::default()
            },
        }
    }

    fn add_test_relic(state: &mut GameState) -> RelicInstanceId {
        let id = RelicInstanceId::new(777);
        let combat = state.combat_mut().unwrap();
        combat.relics.insert(
            id,
            RelicInstance {
                id,
                def: TEST_RELIC,
            },
        );
        combat.player.relics.push(id);
        id
    }

    fn test_orb_passive(
        ctx: &RuleCtx<'_>,
        orb: OrbInstanceId,
        _: OrbTrigger,
        _: Option<CreatureId>,
    ) -> Vec<Effect> {
        let Some(player) = ctx.state.orb(orb).map(|orb| orb.owner) else {
            return Vec::new();
        };
        vec![Effect::GainResource {
            player,
            resource: ResourceKind::Energy,
            amount: 1,
        }]
    }

    fn test_orb_def() -> OrbDef {
        OrbDef {
            id: TEST_ORB,
            loc_key: LocKey::new("orb.test"),
            passive: test_orb_passive,
            evoke: test_orb_passive,
            rules: OrbRules::default(),
        }
    }

    fn osty_redirect(
        ctx: &RuleCtx<'_>,
        power: PowerInstanceId,
        mut calc: UnblockedDamageTargetCalc,
    ) -> UnblockedDamageTargetCalc {
        let Some(instance) = ctx
            .state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
        else {
            return calc;
        };
        if calc.target != instance.owner || calc.amount <= Decimal::from(0) {
            return calc;
        }
        let Some(combat) = ctx.state.combat() else {
            return calc;
        };
        if let Some(osty) = combat.creatures.iter().find(|creature| {
            creature.alive
                && creature.pet_owner == Some(combat.player.id)
                && creature.pet_kind == Some(PlayerPetKind::Osty)
        }) {
            calc.target = osty.id;
        }
        calc
    }

    fn osty_redirect_def() -> PowerDef {
        PowerDef {
            id: TEST_OSTY_REDIRECT,
            loc_key: LocKey::new("power.osty_redirect"),
            rules: PowerRules {
                modify_unblocked_damage_target: Some(osty_redirect),
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
    fn all_enemy_damage_skips_targets_killed_by_an_earlier_hit() {
        let mut state = GameState::single_player_test_combat(
            9,
            Vec::<CombatSetupCard>::new(),
            [
                CombatSetupMonster {
                    model: None,
                    max_hp: 3,
                },
                CombatSetupMonster {
                    model: None,
                    max_hp: 10,
                },
            ],
            50,
            3,
            0,
        );
        let monsters = state.combat().unwrap().monster_ids();
        let fragile = monsters[0];
        let sturdy = monsters[1];
        let mut resolver = EffectResolver::default();

        resolver.enqueue(Effect::DealDamageToAllEnemies(DamageAllEnemiesOp {
            source: None,
            dealer: state.player_creature_id(),
            base_amount: Decimal::from(3),
            kind: DamageKind::Attack,
            flags: DamageFlags {
                ignores_block: false,
            },
            hit_count: 2,
        }));

        let StepResult::Done(log) = resolver.drain(&mut state, &StaticRegistry::empty()) else {
            panic!("expected all-enemy damage to finish");
        };

        let damage_results = log
            .iter()
            .filter_map(|entry| match entry {
                LogEntry::StateChanged(StateChange::DamageApplied(result)) => Some(result),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            damage_results
                .iter()
                .filter(|result| result.target == fragile)
                .count(),
            1
        );
        assert_eq!(
            damage_results
                .iter()
                .filter(|result| result.target == sturdy)
                .count(),
            2
        );
        assert!(!state.creature(fragile).unwrap().alive);
        assert_eq!(state.creature(sturdy).unwrap().hp, 4);
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
    fn manual_and_end_of_turn_discards_emit_distinct_events() {
        let registry = StaticRegistry::empty();
        let mut manual_state = GameState::demo_combat(31);
        let player = manual_state.player_id().unwrap();
        let card = manual_state.combat().unwrap().player.piles.hand[0];
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::DiscardCards {
            player,
            cards: vec![card],
            kind: DiscardKind::Manual,
            then_draw: 0,
        });

        let StepResult::Done(manual_log) = resolver.drain(&mut manual_state, &registry) else {
            panic!("manual discard should finish");
        };
        assert!(manual_state.card_is_in_pile(card, PileKind::Discard));
        assert!(manual_log.iter().any(|entry| matches!(
            entry,
            LogEntry::EventTriggered(Event::CardDiscarded(event))
                if event.card == card && event.kind == DiscardKind::Manual
        )));

        let mut cleanup_state = GameState::demo_combat(32);
        let player = cleanup_state.player_id().unwrap();
        let card = cleanup_state.combat().unwrap().player.piles.hand[0];
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::DiscardHand {
            player,
            kind: DiscardKind::EndOfTurn,
        });

        let StepResult::Done(cleanup_log) = resolver.drain(&mut cleanup_state, &registry) else {
            panic!("end-of-turn discard should finish");
        };
        assert!(cleanup_state.card_is_in_pile(card, PileKind::Discard));
        assert!(cleanup_log.iter().any(|entry| matches!(
            entry,
            LogEntry::EventTriggered(Event::CardDiscarded(event))
                if event.card == card && event.kind == DiscardKind::EndOfTurn
        )));
    }

    #[test]
    fn poison_uses_power_amount_modifiers_and_ticks_on_owner_turn_start() {
        let mut registry = StaticRegistry::default();
        registry.relics.register(test_relic());
        let mut state = GameState::demo_combat(33);
        add_test_relic(&mut state);
        let player_creature = state.player_creature_id().unwrap();
        let enemy = state.combat().unwrap().monster_ids()[0];
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::ApplyPower {
            target: enemy,
            power: POISON_POWER,
            amount: Decimal::from(2),
            source: Some(Source::Creature(player_creature)),
        });

        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        assert_eq!(state.power_amount(enemy, POISON_POWER), 3);

        resolver.enqueue(Effect::Trigger(Event::TurnStarted {
            side: crate::core::state::Side::Monsters,
        }));
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        assert_eq!(state.creature(enemy).unwrap().hp, 27);
        assert_eq!(state.power_amount(enemy, POISON_POWER), 2);
    }

    #[test]
    fn orb_channeling_adds_default_slot_and_passive_count_is_modifiable() {
        let mut registry = StaticRegistry::default();
        registry.orbs.register(test_orb_def());
        registry.relics.register(test_relic());
        let mut state = GameState::demo_combat(34);
        add_test_relic(&mut state);
        let player = state.player_id().unwrap();
        state.combat_mut().unwrap().player.energy = 0;
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::ChannelOrb {
            player,
            orb: TEST_ORB,
            source: None,
        });

        let StepResult::Done(channel_log) = resolver.drain(&mut state, &registry) else {
            panic!("orb channel should finish");
        };
        let orb = state.combat().unwrap().player.orb_queue.orbs[0];
        assert_eq!(state.combat().unwrap().player.orb_queue.slots, 1);
        assert!(channel_log
            .iter()
            .any(|entry| matches!(entry, LogEntry::EventTriggered(Event::OrbChanneled(_)))));

        resolver.enqueue(Effect::TriggerOrbPassive {
            orb,
            trigger: OrbTrigger::AfterTurnStart,
            target: None,
        });
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        assert_eq!(state.combat().unwrap().player.energy, 2);
    }

    #[test]
    fn summoning_osty_creates_grows_and_revives_the_pet() {
        let mut registry = StaticRegistry::default();
        registry.relics.register(test_relic());
        let mut state = GameState::demo_combat(35);
        add_test_relic(&mut state);
        let player = state.player_id().unwrap();
        let mut resolver = EffectResolver::default();

        resolver.enqueue(Effect::SummonOsty {
            player,
            amount: Decimal::from(5),
            source: None,
        });
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        let osty = state
            .combat()
            .unwrap()
            .creatures
            .iter()
            .find(|creature| creature.pet_kind == Some(PlayerPetKind::Osty))
            .unwrap()
            .id;
        assert_eq!(state.creature(osty).unwrap().max_hp, 7);

        resolver.enqueue(Effect::SummonOsty {
            player,
            amount: Decimal::from(3),
            source: None,
        });
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        assert_eq!(state.creature(osty).unwrap().max_hp, 12);

        {
            let osty_state = state.creature_mut(osty).unwrap();
            osty_state.alive = false;
            osty_state.hp = 0;
        }
        resolver.enqueue(Effect::SummonOsty {
            player,
            amount: Decimal::from(4),
            source: None,
        });
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        let osty_state = state.creature(osty).unwrap();
        assert!(osty_state.alive);
        assert_eq!(osty_state.hp, 6);
        assert_eq!(osty_state.max_hp, 6);
    }

    #[test]
    fn osty_redirect_interface_can_move_unblocked_attack_damage() {
        let mut registry = StaticRegistry::default();
        registry.powers.register(osty_redirect_def());
        let mut state = GameState::demo_combat(36);
        let player = state.player_id().unwrap();
        let player_creature = state.player_creature_id().unwrap();
        let enemy = state.combat().unwrap().monster_ids()[0];
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::SummonOsty {
            player,
            amount: Decimal::from(6),
            source: None,
        });
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        let osty = state
            .combat()
            .unwrap()
            .creatures
            .iter()
            .find(|creature| creature.pet_kind == Some(PlayerPetKind::Osty))
            .unwrap()
            .id;
        state
            .apply_power(player_creature, TEST_OSTY_REDIRECT, Decimal::from(1))
            .unwrap();

        resolver.enqueue(Effect::DealDamage(DamageOp {
            source: Some(Source::Creature(enemy)),
            dealer: Some(enemy),
            target: player_creature,
            base_amount: Decimal::from(4),
            kind: DamageKind::Attack,
            flags: DamageFlags {
                ignores_block: false,
            },
        }));
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));
        assert_eq!(state.creature(player_creature).unwrap().hp, 50);
        assert_eq!(state.creature(osty).unwrap().hp, 2);
    }

    #[test]
    fn doom_kills_at_side_turn_end_when_amount_reaches_hp() {
        let registry = StaticRegistry::default();
        let mut state = GameState::demo_combat(37);
        let enemy = state.combat().unwrap().monster_ids()[0];
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::ApplyPower {
            target: enemy,
            power: DOOM_POWER,
            amount: Decimal::from(30),
            source: None,
        });
        resolver.enqueue(Effect::Trigger(Event::TurnEnded {
            side: crate::core::state::Side::Monsters,
        }));

        let StepResult::CombatOver(result, _) = resolver.drain(&mut state, &registry) else {
            panic!("doom should end the one-monster combat");
        };
        assert_eq!(result.outcome, CombatOutcome::Victory);
        assert!(!state.creature(enemy).unwrap().alive);
    }

    #[test]
    fn calamity_generates_attacks_after_player_attacks() {
        let mut registry = StaticRegistry::default();
        registry.cards.register(test_strike_def());
        let mut engine = Engine::with_registry(GameState::demo_combat(39), registry);
        let player = engine.state.player_id().unwrap();
        let player_creature = engine.state.player_creature_id().unwrap();
        engine
            .state
            .apply_power(player_creature, CALAMITY_POWER, Decimal::from(1))
            .unwrap();
        let card = engine.state.combat().unwrap().player.piles.hand[0];
        let target = engine.state.combat().unwrap().monster_ids()[0];

        let StepResult::Done(log) = engine.step(Command::PlayCard {
            player,
            card,
            target: Some(target),
        }) else {
            panic!("calamity attack should resolve");
        };

        assert_eq!(engine.state.combat().unwrap().player.piles.hand.len(), 1);
        assert!(log.iter().any(|entry| matches!(
            entry,
            LogEntry::StateChanged(StateChange::CardMoved {
                from: None,
                reason: MoveReason::Generated,
                ..
            })
        )));
    }

    #[test]
    fn discard_orb_and_poison_mechanics_compose_in_one_resolution_flow() {
        let setup = CombatSetupCard {
            def: STARTER_STRIKE,
            upgraded: false,
            costs: CardCosts::energy(1),
        };
        let mut registry = StaticRegistry::default();
        registry.orbs.register(test_orb_def());
        registry.relics.register(test_relic());
        let mut state = GameState::single_player_test_combat(
            38,
            [setup; 2],
            [CombatSetupMonster {
                model: None,
                max_hp: 20,
            }],
            50,
            3,
            1,
        );
        add_test_relic(&mut state);
        let player = state.player_id().unwrap();
        let player_creature = state.player_creature_id().unwrap();
        let enemy = state.combat().unwrap().monster_ids()[0];
        let discarded = state.combat().unwrap().player.piles.hand[0];
        state.combat_mut().unwrap().player.energy = 0;

        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::ChannelOrb {
            player,
            orb: TEST_ORB,
            source: None,
        });
        resolver.enqueue(Effect::ApplyPower {
            target: enemy,
            power: POISON_POWER,
            amount: Decimal::from(1),
            source: Some(Source::Creature(player_creature)),
        });
        resolver.enqueue(Effect::DiscardCards {
            player,
            cards: vec![discarded],
            kind: DiscardKind::Manual,
            then_draw: 1,
        });

        let StepResult::Done(log) = resolver.drain(&mut state, &registry) else {
            panic!("combined setup should finish");
        };
        assert!(log.iter().any(|entry| matches!(
            entry,
            LogEntry::EventTriggered(Event::CardDiscarded(event))
                if event.kind == DiscardKind::Manual
        )));
        assert_eq!(state.power_amount(enemy, POISON_POWER), 2);
        assert_eq!(state.combat().unwrap().player.piles.hand.len(), 1);

        let orb = state.combat().unwrap().player.orb_queue.orbs[0];
        resolver.enqueue(Effect::TriggerOrbPassive {
            orb,
            trigger: OrbTrigger::AfterTurnStart,
            target: None,
        });
        resolver.enqueue(Effect::Trigger(Event::TurnStarted {
            side: crate::core::state::Side::Monsters,
        }));
        assert!(matches!(
            resolver.drain(&mut state, &registry),
            StepResult::Done(_)
        ));

        assert_eq!(state.combat().unwrap().player.energy, 2);
        assert_eq!(state.creature(enemy).unwrap().hp, 18);
        assert_eq!(state.power_amount(enemy, POISON_POWER), 1);
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
    fn event_listener_effects_are_drained_before_next_listener() {
        let mut registry = StaticRegistry::default();
        registry.cards.register(test_strike_def());
        registry.powers.register(mark_on_card_play_def());
        registry.powers.register(observe_on_card_play_def());

        let mut engine = Engine::with_registry(GameState::demo_combat(18), registry);
        let player = engine.state.player_id().unwrap();
        let player_creature = engine.state.player_creature_id().unwrap();
        engine
            .state
            .apply_power(player_creature, TEST_MARK_ON_CARD_PLAY, Decimal::from(1))
            .unwrap();
        engine
            .state
            .apply_power(player_creature, TEST_OBSERVE_ON_CARD_PLAY, Decimal::from(1))
            .unwrap();
        let card = engine.state.combat().unwrap().player.piles.hand[0];
        let target = engine.state.combat().unwrap().monster_ids()[0];

        let result = engine.step(Command::PlayCard {
            player,
            card,
            target: Some(target),
        });

        assert!(matches!(result, StepResult::Done(_)));
        assert_eq!(engine.state.power_amount(player_creature, TEST_STRENGTH), 1);
        assert_eq!(engine.state.creature(player_creature).unwrap().block, 1);
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
    fn multi_card_choice_preserves_selected_order() {
        let setup = CombatSetupCard {
            def: STARTER_STRIKE,
            upgraded: false,
            costs: CardCosts::energy(1),
        };
        let mut state = GameState::single_player_test_combat(
            17,
            [setup; 4],
            [CombatSetupMonster {
                model: None,
                max_hp: 10,
            }],
            50,
            3,
            4,
        );
        let player = state.player_id().unwrap();
        let selected = state.combat().unwrap().player.piles.hand[0..2].to_vec();
        let mut resolver = EffectResolver::default();
        resolver.enqueue(Effect::SelectHandCards {
            player,
            filter: CardFilter::Any,
            min: 2,
            max: 2,
            prompt: LocKey::new("choice.exhaust_card"),
            source: None,
            on_resolve: ChoiceAction::ExhaustSelectedCards,
        });

        let StepResult::NeedChoice(choice, _) =
            resolver.drain(&mut state, &StaticRegistry::empty())
        else {
            panic!("multi-card selection should request a choice");
        };
        assert_eq!(choice.min, 2);
        assert_eq!(choice.max, 2);
        let second_option = choice
            .options
            .iter()
            .find(|option| option.value == ChoiceValue::Card(selected[1]))
            .unwrap()
            .id;
        let first_option = choice
            .options
            .iter()
            .find(|option| option.value == ChoiceValue::Card(selected[0]))
            .unwrap()
            .id;

        resolver
            .submit_choice(ChoiceResponse {
                request: choice.id,
                options: vec![second_option, first_option],
            })
            .unwrap();
        let StepResult::Done(log) = resolver.drain(&mut state, &StaticRegistry::empty()) else {
            panic!("choice should resolve");
        };

        let exhaust = &state.combat().unwrap().player.piles.exhaust;
        assert_eq!(exhaust.as_slice(), &[selected[1], selected[0]]);
        assert!(log.iter().any(|entry| matches!(
            entry,
            LogEntry::ChoiceResolved(resolution)
                if resolution
                    .selected
                    .iter()
                    .map(|option| option.value)
                    .collect::<Vec<_>>()
                    == vec![ChoiceValue::Card(selected[1]), ChoiceValue::Card(selected[0])]
        )));
        assert!(selected
            .iter()
            .all(|card| !state.card_is_in_pile(*card, PileKind::Hand)));
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
            },
        }));

        let result = resolver.drain(&mut state, &registry);

        assert!(matches!(result, StepResult::Done(_)));
        let creature = state.creature(target).unwrap();
        assert!(creature.alive);
        assert!(creature.hp == 0);
    }
}
