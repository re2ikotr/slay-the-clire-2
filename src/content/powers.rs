use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{LocKey, PowerId, PowerInstanceId};
use crate::core::query::{DamageCalc, Decision, ResourceCostCalc};
use crate::core::rules::RuleCtx;

pub type PowerEventFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, &Event) -> Vec<Effect>;
pub type PowerModifyDamageFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, DamageCalc) -> DamageCalc;
pub type PowerModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, PowerInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type PowerDecisionFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId) -> Decision;

#[derive(Clone)]
pub struct PowerDef {
    pub id: PowerId,
    pub loc_key: LocKey,
    pub hooks: PowerHooks,
}

#[derive(Clone, Default)]
pub struct PowerHooks {
    pub on_event: Option<PowerEventFn>,
    pub modify_damage: Option<PowerModifyDamageFn>,
    pub modify_resource_cost: Option<PowerModifyResourceCostFn>,
    pub should_die: Option<PowerDecisionFn>,
}
