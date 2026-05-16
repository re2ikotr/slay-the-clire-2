use crate::core::effect::{DamageResult, Source};
use crate::core::ids::{CardInstanceId, CreatureId, PlayerId, PowerId, PowerInstanceId};
use crate::core::state::{ResourceKind, Side};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    CombatStarted,
    TurnStarted { side: Side },
    TurnEnded { side: Side },
    CardDrawn(CardDrawn),
    CardPlayStarted(CardPlayStarted),
    CardPlayed(CardPlayed),
    DamageDealt(DamageResult),
    BlockGained(BlockGained),
    PowerApplied(PowerApplied),
    ResourceSpent(ResourceChanged),
    ResourceGained(ResourceChanged),
    CreatureHpChanged(CreatureHpChanged),
    DeathPrevented { creature: CreatureId },
    CreatureDied { creature: CreatureId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardDrawn {
    pub player: PlayerId,
    pub card: CardInstanceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardPlayStarted {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub target: Option<CreatureId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardPlayed {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub target: Option<CreatureId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockGained {
    pub target: CreatureId,
    pub amount: i32,
    pub source: Option<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerApplied {
    pub target: CreatureId,
    pub power: PowerId,
    pub instance: PowerInstanceId,
    pub amount: i32,
    pub source: Option<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceChanged {
    pub player: PlayerId,
    pub resource: ResourceKind,
    pub amount: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreatureHpChanged {
    pub creature: CreatureId,
    pub before: i32,
    pub after: i32,
    pub source: Option<Source>,
}
