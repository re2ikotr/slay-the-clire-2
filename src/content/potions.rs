use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{CreatureId, LocKey, PotionId, PotionInstanceId};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, DamageCalc, Decision, DecisionQuery, HpLossCalc,
    OrbPassiveTriggerCountCalc, OrbValueCalc, PowerAmountCalc, ResourceCostCalc, SummonAmountCalc,
    UnblockedDamageTargetCalc,
};
use crate::core::rules::RuleCtx;

pub type PotionUseFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, Option<CreatureId>) -> Vec<Effect>;
pub type PotionEventFn = for<'a> fn(&RuleCtx<'a>, PotionInstanceId, &Event) -> Vec<Effect>;
pub type PotionModifyDamageFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, DamageCalc) -> DamageCalc;
pub type PotionModifyBlockFn = for<'a> fn(&RuleCtx<'a>, PotionInstanceId, BlockCalc) -> BlockCalc;
pub type PotionModifyHpLossFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, HpLossCalc) -> HpLossCalc;
pub type PotionModifyUnblockedDamageTargetFn = for<'a> fn(
    &RuleCtx<'a>,
    PotionInstanceId,
    UnblockedDamageTargetCalc,
) -> UnblockedDamageTargetCalc;
pub type PotionModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type PotionModifyResultPileFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, CardPlayResultPileCalc) -> CardPlayResultPileCalc;
pub type PotionDecisionFn = for<'a> fn(&RuleCtx<'a>, PotionInstanceId, &DecisionQuery) -> Decision;
pub type PotionModifyPowerAmountFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, PowerAmountCalc) -> PowerAmountCalc;
pub type PotionModifyOrbPassiveCountFn = for<'a> fn(
    &RuleCtx<'a>,
    PotionInstanceId,
    OrbPassiveTriggerCountCalc,
) -> OrbPassiveTriggerCountCalc;
pub type PotionModifyOrbValueFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, OrbValueCalc) -> OrbValueCalc;
pub type PotionModifySummonAmountFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, SummonAmountCalc) -> SummonAmountCalc;

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
    pub modify_hp_loss: Option<PotionModifyHpLossFn>,
    pub modify_unblocked_damage_target: Option<PotionModifyUnblockedDamageTargetFn>,
    pub modify_resource_cost: Option<PotionModifyResourceCostFn>,
    pub modify_card_play_result_pile: Option<PotionModifyResultPileFn>,
    pub modify_power_amount: Option<PotionModifyPowerAmountFn>,
    pub modify_orb_passive_trigger_count: Option<PotionModifyOrbPassiveCountFn>,
    pub modify_orb_value: Option<PotionModifyOrbValueFn>,
    pub modify_summon_amount: Option<PotionModifySummonAmountFn>,
    pub decide: Option<PotionDecisionFn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PotionTarget {
    None,
    Enemy,
    AnyCreature,
}
