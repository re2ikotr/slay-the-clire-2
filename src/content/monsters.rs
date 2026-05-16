use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{CreatureId, LocKey, MonsterId};
use crate::core::query::{BlockCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc};
use crate::core::rules::RuleCtx;

pub type MonsterIntentFn = for<'a> fn(&RuleCtx<'a>, CreatureId) -> MonsterIntent;
pub type MonsterActFn = for<'a> fn(&RuleCtx<'a>, CreatureId) -> Vec<Effect>;
pub type MonsterEventFn = for<'a> fn(&RuleCtx<'a>, CreatureId, &Event) -> Vec<Effect>;
pub type MonsterModifyDamageFn = for<'a> fn(&RuleCtx<'a>, CreatureId, DamageCalc) -> DamageCalc;
pub type MonsterModifyBlockFn = for<'a> fn(&RuleCtx<'a>, CreatureId, BlockCalc) -> BlockCalc;
pub type MonsterModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, CreatureId, ResourceCostCalc) -> ResourceCostCalc;
pub type MonsterDecisionFn = for<'a> fn(&RuleCtx<'a>, CreatureId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct MonsterDef {
    pub id: MonsterId,
    pub loc_key: LocKey,
    pub max_hp: rust_decimal::Decimal,
    pub intent: MonsterIntentFn,
    pub act: MonsterActFn,
    pub rules: MonsterRules,
}

#[derive(Clone, Default)]
pub struct MonsterRules {
    pub on_event: Option<MonsterEventFn>,
    pub modify_damage_additive: Option<MonsterModifyDamageFn>,
    pub modify_damage_multiplicative: Option<MonsterModifyDamageFn>,
    pub modify_damage_cap: Option<MonsterModifyDamageFn>,
    pub modify_block_additive: Option<MonsterModifyBlockFn>,
    pub modify_block_multiplicative: Option<MonsterModifyBlockFn>,
    pub modify_resource_cost: Option<MonsterModifyResourceCostFn>,
    pub decide: Option<MonsterDecisionFn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonsterIntent {
    Attack { amount: rust_decimal::Decimal },
    Block { amount: rust_decimal::Decimal },
    Debuff,
    Unknown,
}
