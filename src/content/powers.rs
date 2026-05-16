use rust_decimal::Decimal;

use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{LocKey, PowerId, PowerInstanceId};
use crate::core::query::{BlockCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc};
use crate::core::rules::RuleCtx;

pub type PowerEventFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, &Event) -> Vec<Effect>;
pub type PowerModifyDamageFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, DamageCalc) -> DamageCalc;
pub type PowerModifyBlockFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, BlockCalc) -> BlockCalc;
pub type PowerModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, PowerInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type PowerDecisionFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct PowerDef {
    pub id: PowerId,
    pub loc_key: LocKey,
    pub rules: PowerRules,
}

#[derive(Clone, Default)]
pub struct PowerRules {
    pub on_event: Option<PowerEventFn>,
    pub modify_damage_additive: Option<PowerModifyDamageFn>,
    pub modify_damage_multiplicative: Option<PowerModifyDamageFn>,
    pub modify_damage_cap: Option<PowerModifyDamageFn>,
    pub modify_block_additive: Option<PowerModifyBlockFn>,
    pub modify_block_multiplicative: Option<PowerModifyBlockFn>,
    pub modify_resource_cost: Option<PowerModifyResourceCostFn>,
    pub decide: Option<PowerDecisionFn>,
}

pub const STRENGTH: PowerId = PowerId::new("STRENGTH");

pub fn strength() -> PowerDef {
    PowerDef {
        id: STRENGTH,
        loc_key: LocKey::new("power.strength"),
        rules: PowerRules {
            modify_damage_additive: Some(strength_modify_damage_additive),
            ..PowerRules::default()
        },
    }
}

fn strength_modify_damage_additive(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = ctx
        .state
        .combat()
        .and_then(|combat| combat.powers.get(&power))
    else {
        return calc;
    };

    if calc.dealer == Some(instance.owner) {
        calc.amount += Decimal::from(instance.amount);
    }

    if calc.amount < Decimal::from(0) {
        calc.amount = Decimal::from(0);
    }

    calc
}
