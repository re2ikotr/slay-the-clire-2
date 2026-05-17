use crate::core::effect::{DamageResult, Effect, MoveReason};
use crate::core::engine::CombatResult;
use crate::core::event::Event;
use crate::core::ids::{CardInstanceId, CreatureId, PlayerId, PowerInstanceId};
use crate::core::query::{Decision, ModifierLog};
use crate::core::state::{PileId, ResourceKind, StateError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogEntry {
    EffectStarted(Effect),
    EventTriggered(Event),
    StateChanged(StateChange),
    ModifierApplied(ModifierLog),
    DecisionMade(Decision),
    ChoiceRequested(crate::core::effect::ChoiceRequest),
    CombatEnded(CombatResult),
    Error(StateError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateChange {
    ResourceSpent {
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    },
    ResourceGained {
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    },
    DamageApplied(DamageResult),
    HpLost {
        target: CreatureId,
        amount: i32,
    },
    Healed {
        target: CreatureId,
        amount: i32,
    },
    MaxHpGained {
        target: CreatureId,
        amount: i32,
    },
    BlockGained {
        target: CreatureId,
        amount: i32,
    },
    BlockCleared {
        target: CreatureId,
        amount: i32,
    },
    PowerApplied {
        target: CreatureId,
        power: PowerInstanceId,
    },
    PowerRemoved {
        power: PowerInstanceId,
    },
    CardUpgraded {
        card: CardInstanceId,
    },
    CardsShuffled {
        player: PlayerId,
        cards: Vec<CardInstanceId>,
    },
    CardMoved {
        card: CardInstanceId,
        from: Option<PileId>,
        to: PileId,
        reason: MoveReason,
    },
    CreatureDied {
        creature: CreatureId,
    },
}
