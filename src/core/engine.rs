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
            Effect::CheckCombatEnd,
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
