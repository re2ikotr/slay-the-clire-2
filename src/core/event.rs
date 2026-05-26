use crate::core::effect::{DamageResult, DiscardKind, Source};
use crate::core::ids::{
    CardInstanceId, CreatureId, OrbId, OrbInstanceId, PlayerId, PowerId, PowerInstanceId,
};
use crate::core::state::{ResourceKind, Side};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    CombatStarted,
    TurnStarted { side: Side },
    TurnEnded { side: Side },
    CardsShuffled(CardsShuffled),
    CardDrawn(CardDrawn),
    CardDiscarded(CardDiscarded),
    CardExhausted(CardExhausted),
    CardUpgraded(CardUpgraded),
    CardPlayStarted(CardPlayStarted),
    CardPlayed(CardPlayed),
    DamageDealt(DamageResult),
    BlockGained(BlockGained),
    PowerApplied(PowerApplied),
    PowerAmountChanged(PowerAmountChanged),
    ResourceSpent(ResourceChanged),
    ResourceGained(ResourceChanged),
    OrbChanneled(OrbChanneled),
    OrbEvoked(OrbEvoked),
    Summoned(Summoned),
    CreatureHpChanged(CreatureHpChanged),
    DeathPrevented { creature: CreatureId },
    CreatureDied { creature: CreatureId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardsShuffled {
    pub player: PlayerId,
    pub cards: Vec<CardInstanceId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardDrawn {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub from_hand_draw: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardDiscarded {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub kind: DiscardKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardExhausted {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub source: Option<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardUpgraded {
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
pub struct PowerAmountChanged {
    pub target: CreatureId,
    pub power: PowerId,
    pub instance: PowerInstanceId,
    pub delta: i32,
    pub source: Option<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceChanged {
    pub player: PlayerId,
    pub resource: ResourceKind,
    pub amount: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrbChanneled {
    pub player: PlayerId,
    pub orb: OrbInstanceId,
    pub orb_def: OrbId,
    pub source: Option<Source>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbEvoked {
    pub player: PlayerId,
    pub orb: OrbInstanceId,
    pub orb_def: OrbId,
    pub removed: bool,
    pub source: Option<Source>,
    pub targets: Vec<CreatureId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summoned {
    pub player: PlayerId,
    pub creature: CreatureId,
    pub amount: i32,
    pub source: Option<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreatureHpChanged {
    pub creature: CreatureId,
    pub before: i32,
    pub after: i32,
    pub source: Option<Source>,
}
