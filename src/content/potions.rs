use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{CreatureId, LocKey, PotionId, PotionInstanceId};
use crate::core::query::{BlockCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc};
use crate::core::rules::RuleCtx;

pub type PotionUseFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, Option<CreatureId>) -> Vec<Effect>;
pub type PotionEventFn = for<'a> fn(&RuleCtx<'a>, PotionInstanceId, &Event) -> Vec<Effect>;
pub type PotionModifyDamageFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, DamageCalc) -> DamageCalc;
pub type PotionModifyBlockFn = for<'a> fn(&RuleCtx<'a>, PotionInstanceId, BlockCalc) -> BlockCalc;
pub type PotionModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type PotionDecisionFn = for<'a> fn(&RuleCtx<'a>, PotionInstanceId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct PotionDef {
    pub id: PotionId,
    pub loc_key: LocKey,
    pub target: PotionTarget,
    pub use_potion: PotionUseFn,
    pub rules: PotionRules,
}

#[derive(Clone, Default)]
pub struct PotionRules {
    pub on_event: Option<PotionEventFn>,
    pub modify_damage_additive: Option<PotionModifyDamageFn>,
    pub modify_damage_multiplicative: Option<PotionModifyDamageFn>,
    pub modify_damage_cap: Option<PotionModifyDamageFn>,
    pub modify_block_additive: Option<PotionModifyBlockFn>,
    pub modify_block_multiplicative: Option<PotionModifyBlockFn>,
    pub modify_resource_cost: Option<PotionModifyResourceCostFn>,
    pub decide: Option<PotionDecisionFn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PotionTarget {
    None,
    Enemy,
    AnyCreature,
}
