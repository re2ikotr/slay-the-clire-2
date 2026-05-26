use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{LocKey, RelicId, RelicInstanceId};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, DamageCalc, Decision, DecisionQuery, HpLossCalc,
    OrbPassiveTriggerCountCalc, OrbValueCalc, PowerAmountCalc, ResourceCostCalc, SummonAmountCalc,
    UnblockedDamageTargetCalc,
};
use crate::core::rules::RuleCtx;

pub type RelicEventFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, &Event) -> Vec<Effect>;
pub type RelicModifyDamageFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, DamageCalc) -> DamageCalc;
pub type RelicModifyBlockFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, BlockCalc) -> BlockCalc;
pub type RelicModifyHpLossFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, HpLossCalc) -> HpLossCalc;
pub type RelicModifyUnblockedDamageTargetFn = for<'a> fn(
    &RuleCtx<'a>,
    RelicInstanceId,
    UnblockedDamageTargetCalc,
) -> UnblockedDamageTargetCalc;
pub type RelicModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type RelicModifyResultPileFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, CardPlayResultPileCalc) -> CardPlayResultPileCalc;
pub type RelicDecisionFn = for<'a> fn(&RuleCtx<'a>, RelicInstanceId, &DecisionQuery) -> Decision;
pub type RelicModifyPowerAmountFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, PowerAmountCalc) -> PowerAmountCalc;
pub type RelicModifyOrbPassiveCountFn = for<'a> fn(
    &RuleCtx<'a>,
    RelicInstanceId,
    OrbPassiveTriggerCountCalc,
) -> OrbPassiveTriggerCountCalc;
pub type RelicModifyOrbValueFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, OrbValueCalc) -> OrbValueCalc;
pub type RelicModifySummonAmountFn =
    for<'a> fn(&RuleCtx<'a>, RelicInstanceId, SummonAmountCalc) -> SummonAmountCalc;

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
    pub modify_hp_loss: Option<RelicModifyHpLossFn>,
    pub modify_unblocked_damage_target: Option<RelicModifyUnblockedDamageTargetFn>,
    pub modify_resource_cost: Option<RelicModifyResourceCostFn>,
    pub modify_card_play_result_pile: Option<RelicModifyResultPileFn>,
    pub modify_power_amount: Option<RelicModifyPowerAmountFn>,
    pub modify_orb_passive_trigger_count: Option<RelicModifyOrbPassiveCountFn>,
    pub modify_orb_value: Option<RelicModifyOrbValueFn>,
    pub modify_summon_amount: Option<RelicModifySummonAmountFn>,
    pub decide: Option<RelicDecisionFn>,
}
