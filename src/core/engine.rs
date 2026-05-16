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

        match command_to_effects(&self.state, command) {
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

fn command_to_effects(state: &GameState, command: Command) -> Result<Vec<Effect>, CommandError> {
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

            let discard = PileId::player(player, PileKind::Discard);
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
                Effect::ExecuteCardBody {
                    player,
                    card,
                    target,
                },
                Effect::MoveCard {
                    card,
                    to: discard,
                    reason: MoveReason::Discard,
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
    use crate::content::cards::{DEFEND_IRONCLAD, STRIKE_IRONCLAD};

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
}
