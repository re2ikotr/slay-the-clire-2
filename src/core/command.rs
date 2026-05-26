use std::fmt;

use crate::core::ids::{CardInstanceId, ChoiceId, CreatureId, PlayerId, PotionInstanceId};
use crate::core::query::PreventReason;
use crate::core::state::Side;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    PlayCard {
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
    },
    EndTurn {
        side: Side,
    },
    UsePotion {
        potion: PotionInstanceId,
        target: Option<CreatureId>,
    },
    Choose {
        request: ChoiceId,
        options: Vec<ChoiceId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    CombatRequired,
    ChoiceRequired,
    UnexpectedChoice,
    ChoiceMismatch {
        expected: ChoiceId,
        actual: ChoiceId,
    },
    ChoiceCountOutOfRange {
        min: usize,
        max: usize,
        actual: usize,
    },
    InvalidChoiceOption(ChoiceId),
    DisabledChoiceOption(ChoiceId),
    DuplicateChoiceOption(ChoiceId),
    InvalidPhase,
    InvalidCard(CardInstanceId),
    Prevented(PreventReason),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CombatRequired => write!(f, "combat is required for this command"),
            Self::ChoiceRequired => write!(f, "a pending choice must be resolved first"),
            Self::UnexpectedChoice => write!(f, "there is no pending choice"),
            Self::ChoiceMismatch { expected, actual } => {
                write!(
                    f,
                    "choice mismatch: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            Self::ChoiceCountOutOfRange { min, max, actual } => write!(
                f,
                "choice selected {actual} options, expected between {min} and {max}"
            ),
            Self::InvalidChoiceOption(option) => write!(f, "invalid choice option: {:?}", option),
            Self::DisabledChoiceOption(option) => {
                write!(f, "disabled choice option: {:?}", option)
            }
            Self::DuplicateChoiceOption(option) => {
                write!(f, "duplicate choice option: {:?}", option)
            }
            Self::InvalidPhase => write!(f, "command is not valid in the current phase"),
            Self::InvalidCard(card) => write!(f, "invalid card: {:?}", card),
            Self::Prevented(reason) => write!(f, "command prevented: {:?}", reason),
        }
    }
}

impl std::error::Error for CommandError {}
