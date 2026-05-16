use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{LocKey, RelicId, RelicInstanceId};
use crate::core::query::{DamageCalc, ResourceCostCalc};
use crate::core::rules::RuleCtx;

pub type RelicEventFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, &Event) -> Vec<Effect>;
pub type RelicModifyDamageFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, DamageCalc) -> DamageCalc;
pub type RelicModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, ResourceCostCalc) -> ResourceCostCalc;

#[derive(Clone)]
pub struct RelicDef {
    pub id: RelicId,
    pub loc_key: LocKey,
    pub hooks: RelicHooks,
}

#[derive(Clone, Default)]
pub struct RelicHooks {
    pub on_event: Option<RelicEventFn>,
    pub modify_damage: Option<RelicModifyDamageFn>,
    pub modify_resource_cost: Option<RelicModifyResourceCostFn>,
}
