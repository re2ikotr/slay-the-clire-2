use crate::content::cards::{CardKeyword, CardType};
use crate::content::powers::CORRUPTION_POWER;
use crate::core::command::{Command, CommandError};
use crate::core::effect::{Effect, MoveReason};
use crate::core::event::{CardPlayStarted, CardPlayed, Event};
use crate::core::log::LogEntry;
use crate::core::resolver::EffectResolver;
use crate::core::state::{CombatPhase, GameState, PileId, PileKind, Side};
use crate::registry::StaticRegistry;

pub struct Engine {
    pub state: GameState,
    pub registry: StaticRegistry,
    resolver: EffectResolver,
}

impl Engine {
    pub fn new(state: GameState) -> Self {
        Self::with_registry(state, StaticRegistry::default())
    }

    pub fn with_registry(state: GameState, registry: StaticRegistry) -> Self {
        Self {
            state,
            registry,
            resolver: EffectResolver::default(),
        }
    }

    pub fn step(&mut self, command: Command) -> StepResult {
        if self.resolver.has_pending_choice() {
            return match command {
                Command::Choose { choice } => match self.resolver.submit_choice(choice) {
                    Ok(()) => self.resolver.drain(&mut self.state, &self.registry),
                    Err(error) => StepResult::Rejected(error, self.resolver.take_log()),
                },
                _ => StepResult::Rejected(CommandError::ChoiceRequired, self.resolver.take_log()),
            };
        }

        if matches!(command, Command::Choose { .. }) {
            return StepResult::Rejected(CommandError::UnexpectedChoice, self.resolver.take_log());
        }

        match command_to_effects(&self.state, &self.registry, command) {
            Ok(effects) => {
                self.resolver.enqueue_all(effects);
                self.resolver.drain(&mut self.state, &self.registry)
            }
            Err(error) => StepResult::Rejected(error, self.resolver.take_log()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepResult {
    Done(Vec<LogEntry>),
    NeedChoice(crate::core::effect::ChoiceRequest, Vec<LogEntry>),
    CombatOver(CombatResult, Vec<LogEntry>),
    Rejected(CommandError, Vec<LogEntry>),
    Failed(crate::core::state::StateError, Vec<LogEntry>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatResult {
    pub outcome: CombatOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatOutcome {
    Victory,
    Defeat,
}

fn command_to_effects(
    state: &GameState,
    registry: &StaticRegistry,
    command: Command,
) -> Result<Vec<Effect>, CommandError> {
    match command {
        Command::PlayCard {
            player,
            card,
            target,
        } => {
            let combat = state.combat().ok_or(CommandError::CombatRequired)?;
            if combat.phase != CombatPhase::PlayerAction {
                return Err(CommandError::InvalidPhase);
            }
            if !combat.cards.contains_key(&card) {
                return Err(CommandError::InvalidCard(card));
            }

            let play = PileId::player(player, PileKind::Play);
            let (result_pile, result_reason) = card_result_pile(state, registry, player, card)?;
            Ok(vec![
                Effect::ValidateCardPlay {
                    player,
                    card,
                    target,
                },
                Effect::Trigger(Event::CardPlayStarted(CardPlayStarted {
                    player,
                    card,
                    target,
                })),
                Effect::PayCardCosts { player, card },
                Effect::MoveCard {
                    card,
                    to: play,
                    reason: MoveReason::Play,
                },
                Effect::ExecuteCardBody {
                    player,
                    card,
                    target,
                },
                Effect::MoveCard {
                    card,
                    to: result_pile,
                    reason: result_reason,
                },
                Effect::Trigger(Event::CardPlayed(CardPlayed {
                    player,
                    card,
                    target,
                })),
                Effect::CheckDeaths,
                Effect::CheckCombatEnd,
            ])
        }
        Command::EndTurn { side } => Ok(vec![
            Effect::Trigger(Event::TurnEnded { side }),
            Effect::EndTurn(side),
        ]),
        Command::UsePotion { .. } => Ok(vec![]),
        Command::Choose { .. } => Err(CommandError::UnexpectedChoice),
    }
}

fn card_result_pile(
    state: &GameState,
    registry: &StaticRegistry,
    player: crate::core::ids::PlayerId,
    card: crate::core::ids::CardInstanceId,
) -> Result<(PileId, MoveReason), CommandError> {
    let card_state = state.card(card).ok_or(CommandError::InvalidCard(card))?;
    let Some(def) = registry.cards.get(card_state.def) else {
        return Ok((
            PileId::player(player, PileKind::Discard),
            MoveReason::Discard,
        ));
    };

    let corruption_exhausts_skill = def.card_type == CardType::Skill
        && state
            .player_creature_id()
            .map(|creature| state.has_power(creature, CORRUPTION_POWER))
            .unwrap_or(false);

    if def.card_type == CardType::Power || card_state.flags.purge_on_use {
        Ok((
            PileId::player(player, PileKind::Removed),
            MoveReason::Removed,
        ))
    } else if def.has_keyword(card_state.upgraded, CardKeyword::Exhaust)
        || corruption_exhausts_skill
    {
        Ok((
            PileId::player(player, PileKind::Exhaust),
            MoveReason::Exhaust,
        ))
    } else {
        Ok((
            PileId::player(player, PileKind::Discard),
            MoveReason::Discard,
        ))
    }
}

pub(crate) fn combat_result_for_state(state: &GameState) -> Option<CombatResult> {
    let combat = state.combat()?;
    let player_dead = state
        .creature(combat.player.creature)
        .map(|creature| !creature.alive)
        .unwrap_or(false);
    if player_dead {
        return Some(CombatResult {
            outcome: CombatOutcome::Defeat,
        });
    }

    let any_alive_monster = combat
        .creatures
        .iter()
        .any(|creature| creature.side == Side::Monsters && creature.alive);
    if !any_alive_monster {
        return Some(CombatResult {
            outcome: CombatOutcome::Victory,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::content::cards::{
        BASH, BATTLE_TRANCE, BLOODLETTING, DEFEND_IRONCLAD, POMMEL_STRIKE, STRIKE_IRONCLAD,
    };
    use crate::content::powers::{NO_DRAW_POWER, VULNERABLE};
    use crate::core::event::Event;
    use crate::core::ids::CardId;
    use crate::core::ids::{CardInstanceId, CreatureId};
    use crate::core::log::{LogEntry, StateChange};
    use crate::core::query::PreventReason;
    use crate::core::state::{PileId, PileKind, ResourceKind};

    use super::*;

    #[test]
    fn basic_nibbit_combat_runs_back_to_player_action() {
        let mut engine = Engine::new(GameState::basic_nibbit_combat(1));
        let player = engine.state.player_id().unwrap();
        let player_creature = engine.state.player_creature_id().unwrap();
        let enemy = engine.state.combat().unwrap().monster_ids()[0];
        let combat = engine.state.combat().unwrap();
        let strike = combat
            .player
            .piles
            .hand
            .iter()
            .copied()
            .find(|card| combat.cards.get(card).unwrap().def == STRIKE_IRONCLAD)
            .unwrap();
        let defend = combat
            .player
            .piles
            .hand
            .iter()
            .copied()
            .find(|card| combat.cards.get(card).unwrap().def == DEFEND_IRONCLAD)
            .unwrap();

        assert!(matches!(
            engine.step(Command::PlayCard {
                player,
                card: defend,
                target: None,
            }),
            StepResult::Done(_)
        ));
        assert_eq!(engine.state.creature(player_creature).unwrap().block, 5);

        assert!(matches!(
            engine.step(Command::PlayCard {
                player,
                card: strike,
                target: Some(enemy),
            }),
            StepResult::Done(_)
        ));
        assert_eq!(engine.state.creature(enemy).unwrap().hp, 36);

        assert!(matches!(
            engine.step(Command::EndTurn { side: Side::Player }),
            StepResult::Done(_)
        ));

        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.phase, CombatPhase::PlayerAction);
        assert_eq!(combat.player.energy, combat.player.max_energy);
        assert_eq!(combat.player.piles.hand.len(), 2);

        let player_creature = engine.state.creature(player_creature).unwrap();
        assert_eq!(player_creature.hp, 43);
        assert_eq!(player_creature.block, 0);
        assert_eq!(engine.state.creature(enemy).unwrap().turns_taken, 1);
    }

    #[test]
    fn rejected_card_play_does_not_leave_stale_effects() {
        let mut engine = Engine::new(GameState::basic_nibbit_combat(2));
        let player = engine.state.player_id().unwrap();
        let enemy = engine.state.combat().unwrap().monster_ids()[0];
        let combat = engine.state.combat_mut().unwrap();
        combat.player.energy = 0;
        let strike = combat
            .player
            .piles
            .hand
            .iter()
            .copied()
            .find(|card| combat.cards.get(card).unwrap().def == STRIKE_IRONCLAD)
            .unwrap();

        let rejected = engine.step(Command::PlayCard {
            player,
            card: strike,
            target: Some(enemy),
        });

        assert!(matches!(
            rejected,
            StepResult::Rejected(
                CommandError::Prevented(PreventReason::InsufficientResource(ResourceKind::Energy)),
                _
            )
        ));
        {
            let combat = engine.state.combat().unwrap();
            assert_eq!(combat.player.energy, 0);
            assert!(combat.player.piles.hand.contains(&strike));
        }

        engine.state.combat_mut().unwrap().player.energy = 1;
        let played = engine.step(Command::PlayCard {
            player,
            card: strike,
            target: Some(enemy),
        });

        assert!(matches!(played, StepResult::Done(_)));
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.energy, 0);
        assert!(combat.player.piles.discard.contains(&strike));
        assert_eq!(engine.state.creature(enemy).unwrap().hp, 36);
    }

    #[test]
    fn full_nibbit_combat_runs_to_victory_after_shuffle() {
        let mut engine = Engine::new(GameState::full_nibbit_combat(4));
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.piles.hand.len(), 2);
        assert_eq!(combat.player.piles.draw.len(), 7);

        let enemy = combat.monster_ids()[0];
        let player_creature = combat.player.creature;
        let (outcome, log) = run_auto_nibbit_combat(&mut engine);

        assert_eq!(outcome, CombatOutcome::Victory);
        assert!(!engine.state.creature(enemy).unwrap().alive);
        assert!(log.iter().any(|entry| matches!(
            entry,
            LogEntry::StateChanged(StateChange::CardsShuffled { .. })
        )));
        assert!(log
            .iter()
            .any(|entry| matches!(entry, LogEntry::EventTriggered(Event::CardsShuffled(_)))));
        assert!(log.iter().any(|entry| matches!(
            entry,
            LogEntry::StateChanged(StateChange::ResourceGained {
                resource: ResourceKind::Energy,
                ..
            })
        )));
        assert!(log.iter().any(|entry| matches!(
            entry,
            LogEntry::StateChanged(StateChange::DamageApplied(result))
                if result.dealer == Some(enemy) && result.target == player_creature
        )));
    }

    #[test]
    fn bash_damages_nibbit_and_applies_vulnerable() {
        let mut engine = Engine::new(GameState::basic_nibbit_combat(31));
        let player = engine.state.player_id().unwrap();
        let enemy = engine.state.combat().unwrap().monster_ids()[0];
        let bash = add_card_to_hand(&mut engine, BASH, false);

        let result = engine.step(Command::PlayCard {
            player,
            card: bash,
            target: Some(enemy),
        });

        assert!(matches!(result, StepResult::Done(_)));
        assert_eq!(engine.state.creature(enemy).unwrap().hp, 34);
        assert_eq!(engine.state.power_amount(enemy, VULNERABLE), 2);
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.energy, 1);
        assert!(combat.player.piles.discard.contains(&bash));
    }

    #[test]
    fn pommel_strike_damages_nibbit_and_draws_card() {
        let mut engine = Engine::new(GameState::full_nibbit_combat(32));
        let player = engine.state.player_id().unwrap();
        let enemy = engine.state.combat().unwrap().monster_ids()[0];
        let pommel = add_card_to_hand(&mut engine, POMMEL_STRIKE, false);
        let before = engine.state.combat().unwrap().player.piles.draw.len();

        let result = engine.step(Command::PlayCard {
            player,
            card: pommel,
            target: Some(enemy),
        });

        assert!(matches!(result, StepResult::Done(_)));
        assert_eq!(engine.state.creature(enemy).unwrap().hp, 33);
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.piles.draw.len(), before - 1);
        assert_eq!(combat.player.piles.hand.len(), 3);
        assert!(combat.player.piles.discard.contains(&pommel));
    }

    #[test]
    fn battle_trance_draws_then_blocks_more_draws_this_turn() {
        let mut engine = Engine::new(GameState::full_nibbit_combat(33));
        let player = engine.state.player_id().unwrap();
        let player_creature = engine.state.player_creature_id().unwrap();
        let battle_trance = add_card_to_hand(&mut engine, BATTLE_TRANCE, false);

        let result = engine.step(Command::PlayCard {
            player,
            card: battle_trance,
            target: None,
        });

        assert!(matches!(result, StepResult::Done(_)));
        assert_eq!(engine.state.power_amount(player_creature, NO_DRAW_POWER), 1);
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.energy, 3);
        assert_eq!(combat.player.piles.hand.len(), 5);
        assert_eq!(combat.player.piles.draw.len(), 4);
        assert!(combat.player.piles.discard.contains(&battle_trance));
    }

    #[test]
    fn bloodletting_loses_hp_and_gains_energy_against_nibbit() {
        let mut engine = Engine::new(GameState::basic_nibbit_combat(34));
        let player = engine.state.player_id().unwrap();
        let player_creature = engine.state.player_creature_id().unwrap();
        let bloodletting = add_card_to_hand(&mut engine, BLOODLETTING, false);

        let result = engine.step(Command::PlayCard {
            player,
            card: bloodletting,
            target: None,
        });

        assert!(matches!(result, StepResult::Done(_)));
        assert_eq!(engine.state.creature(player_creature).unwrap().hp, 47);
        let combat = engine.state.combat().unwrap();
        assert_eq!(combat.player.energy, 5);
        assert!(combat.player.piles.discard.contains(&bloodletting));
    }

    fn run_auto_nibbit_combat(engine: &mut Engine) -> (CombatOutcome, Vec<LogEntry>) {
        let player = engine.state.player_id().unwrap();
        let enemy = engine.state.combat().unwrap().monster_ids()[0];
        let mut all_logs = Vec::new();

        for _ in 0..20 {
            while let Some((card, target)) = next_auto_card(engine, enemy) {
                match engine.step(Command::PlayCard {
                    player,
                    card,
                    target,
                }) {
                    StepResult::Done(log) => all_logs.extend(log),
                    StepResult::CombatOver(result, log) => {
                        all_logs.extend(log);
                        return (result.outcome, all_logs);
                    }
                    other => panic!("unexpected play-card result: {other:?}"),
                }
            }

            match engine.step(Command::EndTurn { side: Side::Player }) {
                StepResult::Done(log) => all_logs.extend(log),
                StepResult::CombatOver(result, log) => {
                    all_logs.extend(log);
                    return (result.outcome, all_logs);
                }
                other => panic!("unexpected end-turn result: {other:?}"),
            }
        }

        panic!("combat did not finish within the smoke-test turn limit");
    }

    fn next_auto_card(
        engine: &Engine,
        enemy: CreatureId,
    ) -> Option<(CardInstanceId, Option<CreatureId>)> {
        let combat = engine.state.combat()?;
        if combat.phase != CombatPhase::PlayerAction || combat.player.energy <= 0 {
            return None;
        }

        let enemy_alive = engine
            .state
            .creature(enemy)
            .map(|creature| creature.alive)
            .unwrap_or(false);

        let mut fallback = None;
        for card in &combat.player.piles.hand {
            let card_state = combat.cards.get(card)?;
            let costs = card_state.effective_costs();
            let Some(energy) = costs.energy.amount_to_pay(combat.player.energy) else {
                continue;
            };
            let Some(stars) = costs.stars.amount_to_pay(combat.player.stars) else {
                continue;
            };
            if energy > combat.player.energy || stars > combat.player.stars {
                continue;
            }

            if card_state.def == STRIKE_IRONCLAD && enemy_alive {
                return Some((*card, Some(enemy)));
            }
            if card_state.def == DEFEND_IRONCLAD {
                fallback.get_or_insert((*card, None));
            }
        }

        fallback
    }

    fn add_card_to_hand(engine: &mut Engine, def: CardId, upgraded: bool) -> CardInstanceId {
        let player = engine.state.player_id().unwrap();
        let to = PileId::player(player, PileKind::Hand);
        let costs = engine.registry.cards.get(def).unwrap().costs_for(upgraded);
        engine
            .state
            .add_generated_card(player, def, to, upgraded, costs, false, false)
            .unwrap()
    }
}
