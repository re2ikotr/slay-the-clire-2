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
    BlockGained {
        target: CreatureId,
        amount: rust_decimal::Decimal,
    },
    PowerApplied {
        target: CreatureId,
        power: PowerInstanceId,
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
