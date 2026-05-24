use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{LocKey, RelicId, RelicInstanceId};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc,
};
use crate::core::rules::RuleCtx;

pub type RelicEventFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, &Event) -> Vec<Effect>;
pub type RelicModifyDamageFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, DamageCalc) -> DamageCalc;
pub type RelicModifyBlockFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, BlockCalc) -> BlockCalc;
pub type RelicModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type RelicModifyResultPileFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, CardPlayResultPileCalc) -> CardPlayResultPileCalc;
pub type RelicDecisionFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct RelicDef {
    pub id: RelicId,
    pub loc_key: LocKey,
    pub rules: RelicRules,
}

#[derive(Clone, Default)]
pub struct RelicRules {
    pub on_event: Option<RelicEventFn>,
    pub modify_damage_additive: Option<RelicModifyDamageFn>,
    pub modify_damage_multiplicative: Option<RelicModifyDamageFn>,
    pub modify_damage_cap: Option<RelicModifyDamageFn>,
    pub modify_block_additive: Option<RelicModifyBlockFn>,
    pub modify_block_multiplicative: Option<RelicModifyBlockFn>,
    pub modify_resource_cost: Option<RelicModifyResourceCostFn>,
    pub modify_card_play_result_pile: Option<RelicModifyResultPileFn>,
    pub decide: Option<RelicDecisionFn>,
}
