use rust_decimal::Decimal;

use crate::core::event::Event;
use crate::core::ids::{
    CardInstanceId, ChoiceId, CreatureId, LocKey, PlayerId, PotionInstanceId, PowerId,
    PowerInstanceId, RelicInstanceId,
};
use crate::core::state::{CombatPhase, PileId, ResourceKind, Side};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Trigger(Event),
    ValidateCardPlay {
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
    },
    SpendResource {
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    },
    GainResource {
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    },
    PayCardCosts {
        player: PlayerId,
        card: CardInstanceId,
    },
    ExecuteCardBody {
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
    },
    DealDamage(DamageOp),
    GainBlock {
        target: CreatureId,
        amount: Decimal,
        source: Option<Source>,
    },
    ApplyPower {
        target: CreatureId,
        power: PowerId,
        amount: Decimal,
        source: Option<Source>,
    },
    DrawCards {
        player: PlayerId,
        count: u8,
    },
    MoveCard {
        card: CardInstanceId,
        to: PileId,
        reason: MoveReason,
    },
    CheckDeaths,
    CheckCombatEnd,
    StartTurn(Side),
    EndTurn(Side),
    EnterPhase(CombatPhase),
    RequestChoice(ChoiceRequest),
    ResolveChoice(ChoiceId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Card(CardInstanceId),
    Power(PowerInstanceId),
    Relic(RelicInstanceId),
    Potion(PotionInstanceId),
    Creature(CreatureId),
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageKind {
    Attack,
    Power,
    Thorns,
    LifeLoss,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageFlags {
    pub ignores_block: bool,
    pub is_attack: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageOp {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub base_amount: Decimal,
    pub kind: DamageKind,
    pub flags: DamageFlags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageResult {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub kind: DamageKind,
    pub requested: Decimal,
    pub blocked: Decimal,
    pub hp_loss: Decimal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveReason {
    Draw,
    Discard,
    Exhaust,
    Play,
    Generated,
    Cleanup,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceRequest {
    pub id: ChoiceId,
    pub kind: ChoiceKind,
    pub source: Option<Source>,
    pub prompt: LocKey,
    pub options: Vec<ChoiceOption>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceKind {
    SelectCard,
    SelectTarget,
    SelectReward,
    Generic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceOption {
    pub id: ChoiceId,
    pub loc_key: LocKey,
    pub enabled: bool,
}
