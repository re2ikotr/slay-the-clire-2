use std::collections::VecDeque;

use rust_decimal::Decimal;

use crate::core::command::CommandError;
use crate::core::effect::{DamageOp, DamageResult, Effect};
use crate::core::engine::{combat_result_for_state, StepResult};
use crate::core::event::{
    BlockGained, CardDrawn, CreatureHpChanged, Event, PowerApplied, ResourceChanged,
};
use crate::core::ids::ChoiceId;
use crate::core::log::{LogEntry, StateChange};
use crate::core::query::{BlockCalc, DamageCalc, Decision, PreventReason, ResourceCostCalc};
use crate::core::rules::RulePipeline;
use crate::core::state::{CardCost, CombatPhase, GameState, ResourceKind, StateError};

#[derive(Default)]
pub struct EffectResolver {
    queue: VecDeque<Effect>,
    pending_choice: Option<crate::core::effect::ChoiceRequest>,
    log: Vec<LogEntry>,
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

    pub fn drain(&mut self, state: &mut GameState) -> StepResult {
        while let Some(effect) = self.queue.pop_front() {
            self.log.push(LogEntry::EffectStarted(effect.clone()));

            match self.apply_effect(state, effect) {
                ApplyResult::Continue(more) => self.queue.extend(more),
                ApplyResult::NeedChoice(choice) => {
                    self.pending_choice = Some(choice.clone());
                    self.log.push(LogEntry::ChoiceRequested(choice.clone()));
                    return StepResult::NeedChoice(choice, self.take_log());
                }
                ApplyResult::CombatOver(result) => {
                    self.log.push(LogEntry::CombatEnded(result.clone()));
                    return StepResult::CombatOver(result, self.take_log());
                }
                ApplyResult::Rejected(error) => {
                    return StepResult::Rejected(error, self.take_log());
                }
                ApplyResult::StateError(error) => {
                    self.log.push(LogEntry::Error(error.clone()));
                    return StepResult::Failed(error, self.take_log());
                }
            }
        }

        StepResult::Done(self.take_log())
    }

    fn apply_effect(&mut self, state: &mut GameState, effect: Effect) -> ApplyResult {
        match effect {
            Effect::Trigger(event) => {
                self.log.push(LogEntry::EventTriggered(event.clone()));
                ApplyResult::Continue(RulePipeline::notify(state, &event))
            }
            Effect::ValidateCardPlay {
                player,
                card,
                target,
            } => {
                let decision = RulePipeline::should_play(state, card, target);
                self.log.push(LogEntry::DecisionMade(decision.clone()));
                match decision {
                    Decision::Allow => {
                        match self.resolve_card_payment(state, player, card, false) {
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
                match self.resolve_card_payment(state, player, card, true) {
                    Ok(payment) => self.spend_card_payment(state, player, payment),
                    Err(error) => ApplyResult::Rejected(error),
                }
            }
            Effect::ExecuteCardBody { .. } => ApplyResult::Continue(Vec::new()),
            Effect::DealDamage(op) => self.apply_damage(state, op),
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
                let (calc, modifiers) = RulePipeline::modify_block(state, calc);
                self.log
                    .extend(modifiers.into_iter().map(LogEntry::ModifierApplied));
                match state.gain_block(target, calc.amount) {
                    Ok(actual) => {
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
                Ok(instance) => {
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
                            amount,
                            source,
                        },
                    ))])
                }
                Err(error) => ApplyResult::StateError(error),
            },
            Effect::DrawCards { player, count } => match state.draw_cards(player, count) {
                Ok(cards) => ApplyResult::Continue(
                    cards
                        .into_iter()
                        .map(|card| Effect::Trigger(Event::CardDrawn(CardDrawn { player, card })))
                        .collect(),
                ),
                Err(error) => ApplyResult::StateError(error),
            },
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
            Effect::CheckDeaths => self.check_deaths(state),
            Effect::CheckCombatEnd => combat_result_for_state(state)
                .map(ApplyResult::CombatOver)
                .unwrap_or_else(|| ApplyResult::Continue(Vec::new())),
            Effect::StartTurn(side) => {
                let phase = match side {
                    crate::core::state::Side::Player => CombatPhase::PlayerStart,
                    crate::core::state::Side::Monsters => CombatPhase::EnemyAction,
                };
                match state.set_phase(phase) {
                    Ok(()) => {
                        ApplyResult::Continue(vec![Effect::Trigger(Event::TurnStarted { side })])
                    }
                    Err(error) => ApplyResult::StateError(error),
                }
            }
            Effect::EndTurn(side) => {
                let phase = match side {
                    crate::core::state::Side::Player => CombatPhase::PlayerEnd,
                    crate::core::state::Side::Monsters => CombatPhase::EnemyEnd,
                };
                match state.set_phase(phase) {
                    Ok(()) => ApplyResult::Continue(Vec::new()),
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

    fn apply_damage(&mut self, state: &mut GameState, op: DamageOp) -> ApplyResult {
        let calc = DamageCalc {
            source: op.source,
            dealer: op.dealer,
            target: op.target,
            kind: op.kind,
            base_amount: op.base_amount,
            amount: op.base_amount,
        };
        let (calc, modifiers) = RulePipeline::modify_damage(state, calc);
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
        let mut blocked = Decimal::from(0);
        let mut hp_loss = requested;

        if !op.flags.ignores_block {
            blocked = if creature.block < requested {
                creature.block
            } else {
                requested
            };
            creature.block -= blocked;
            hp_loss = requested - blocked;
        }

        creature.hp -= hp_loss;
        let hp_after = creature.hp;

        let result = DamageResult {
            source: op.source,
            dealer: op.dealer,
            target,
            kind: op.kind,
            requested,
            blocked,
            hp_loss,
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

    fn resolve_card_payment(
        &mut self,
        state: &GameState,
        player: crate::core::ids::PlayerId,
        card: crate::core::ids::CardInstanceId,
        log_modifiers: bool,
    ) -> Result<CardPayment, CommandError> {
        let card_state = state.card(card).ok_or(CommandError::InvalidCard(card))?;
        let costs = card_state.effective_costs();

        Ok(CardPayment {
            energy: self.resolve_resource_cost(
                state,
                player,
                card,
                ResourceKind::Energy,
                costs.energy,
                log_modifiers,
            )?,
            stars: self.resolve_resource_cost(
                state,
                player,
                card,
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
            let (calc, modifiers) = RulePipeline::modify_resource_cost(state, calc);
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

    fn check_deaths(&mut self, state: &mut GameState) -> ApplyResult {
        let mut effects = Vec::new();
        for creature in state.death_candidates() {
            let decision = RulePipeline::should_die(state, creature);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CardPayment {
    energy: i32,
    stars: i32,
}

fn command_error_from_state(error: StateError) -> CommandError {
    match error {
        StateError::CombatNotActive => CommandError::CombatRequired,
        _ => CommandError::Prevented(PreventReason::CannotPlay),
    }
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

    use crate::core::effect::{DamageFlags, DamageKind, DamageOp, Effect};
    use crate::core::engine::{CombatOutcome, Engine, StepResult};
    use crate::core::state::{CardCost, GameState};
    use crate::core::Command;

    use super::EffectResolver;

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

        match resolver.drain(&mut state) {
            StepResult::CombatOver(result, _) => {
                assert_eq!(result.outcome, CombatOutcome::Victory);
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
}
