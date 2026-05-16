use rust_decimal::Decimal;

use crate::core::effect::{DamageKind, Source};
use crate::core::ids::{CardInstanceId, CreatureId, PlayerId};
use crate::core::listener::ListenerRef;
use crate::core::state::ResourceKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    ModifyDamage(DamageCalc),
    ModifyBlock(BlockCalc),
    ModifyResourceCost(ResourceCostCalc),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageCalc {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub kind: DamageKind,
    pub base_amount: Decimal,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockCalc {
    pub source: Option<Source>,
    pub target: CreatureId,
    pub base_amount: Decimal,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceCostCalc {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub resource: ResourceKind,
    pub base_cost: i32,
    pub cost: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Prevent {
        by: ListenerRef,
        reason: PreventReason,
    },
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreventReason {
    CannotPlay,
    CannotDraw,
    CannotDie,
    NoValidTarget,
    InsufficientResource(ResourceKind),
    Custom(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifierLog {
    pub listener: ListenerRef,
    pub phase: ModifierPhase,
    pub before: Decimal,
    pub after: Decimal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierPhase {
    Additive,
    Multiplicative,
    Capping,
    Replacement,
}
