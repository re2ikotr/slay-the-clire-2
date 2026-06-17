use crate::core::effect::{DamageResult, Effect, MoveReason};
use crate::core::engine::CombatResult;
use crate::core::event::Event;
use crate::core::ids::{CardInstanceId, CreatureId, OrbInstanceId, PlayerId, PowerInstanceId};
use crate::core::query::{CardPlayResultPileModifierLog, Decision, ModifierLog};
use crate::core::state::{CardCounter, PileId, PowerCounter, ResourceKind, StateError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogEntry {
    EffectStarted(Effect),
    EventTriggered(Event),
    StateChanged(StateChange),
    ModifierApplied(ModifierLog),
    CardPlayResultPileModified(CardPlayResultPileModifierLog),
    DecisionMade(Decision),
    ChoiceRequested(crate::core::effect::ChoiceRequest),
    ChoiceResolved(crate::core::effect::ChoiceResolution),
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
    OrbSlotCountChanged {
        player: PlayerId,
        slots: u8,
    },
    OrbChanneled {
        orb: OrbInstanceId,
    },
    OrbAmountChanged {
        orb: OrbInstanceId,
        amount: i32,
    },
    OrbEvoked {
        orb: OrbInstanceId,
        removed: bool,
    },
    CardUpgraded {
        card: CardInstanceId,
    },
    CardCounterChanged {
        card: CardInstanceId,
        counter: CardCounter,
        value: i32,
    },
    CardRetainChanged {
        card: CardInstanceId,
        retained: bool,
    },
    PowerCounterChanged {
        power: PowerInstanceId,
        counter: PowerCounter,
        value: i32,
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
