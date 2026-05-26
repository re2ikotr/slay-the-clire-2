use rust_decimal::Decimal;

use crate::core::effect::{Effect, OrbTrigger};
use crate::core::ids::{LocKey, OrbId, OrbInstanceId};
use crate::core::query::{
    OrbPassiveTriggerCountCalc, OrbValueCalc, PowerAmountCalc, SummonAmountCalc,
};
use crate::core::rules::RuleCtx;
use crate::registry::DefRegistry;

pub type OrbActionFn =
    for<'a> fn(&RuleCtx<'a>, OrbInstanceId, Option<crate::core::ids::CreatureId>) -> Vec<Effect>;
pub type OrbModifyPowerAmountFn =
    for<'a> fn(&RuleCtx<'a>, OrbInstanceId, PowerAmountCalc) -> PowerAmountCalc;
pub type OrbModifyPassiveCountFn = for<'a> fn(
    &RuleCtx<'a>,
    OrbInstanceId,
    OrbPassiveTriggerCountCalc,
) -> OrbPassiveTriggerCountCalc;
pub type OrbModifyValueFn = for<'a> fn(&RuleCtx<'a>, OrbInstanceId, OrbValueCalc) -> OrbValueCalc;
pub type OrbModifySummonAmountFn =
    for<'a> fn(&RuleCtx<'a>, OrbInstanceId, SummonAmountCalc) -> SummonAmountCalc;

#[derive(Clone)]
pub struct OrbDef {
    pub id: OrbId,
    pub loc_key: LocKey,
    pub passive: OrbActionFn,
    pub evoke: OrbActionFn,
    pub rules: OrbRules,
}

#[derive(Clone, Default)]
pub struct OrbRules {
    pub modify_power_amount: Option<OrbModifyPowerAmountFn>,
    pub modify_orb_passive_trigger_count: Option<OrbModifyPassiveCountFn>,
    pub modify_orb_value: Option<OrbModifyValueFn>,
    pub modify_summon_amount: Option<OrbModifySummonAmountFn>,
}

pub const LIGHTNING_ORB: OrbId = OrbId::new("LIGHTNING_ORB");
pub const FROST_ORB: OrbId = OrbId::new("FROST_ORB");
pub const DARK_ORB: OrbId = OrbId::new("DARK_ORB");
pub const PLASMA_ORB: OrbId = OrbId::new("PLASMA_ORB");

pub fn register_core_orbs(registry: &mut DefRegistry<OrbId, OrbDef>) {
    for def in [lightning(), frost(), dark(), plasma()] {
        registry.register(def);
    }
}

fn lightning() -> OrbDef {
    OrbDef {
        id: LIGHTNING_ORB,
        loc_key: LocKey::new("orb.lightning"),
        passive: no_orb_action,
        evoke: no_orb_action,
        rules: OrbRules::default(),
    }
}

fn frost() -> OrbDef {
    OrbDef {
        id: FROST_ORB,
        loc_key: LocKey::new("orb.frost"),
        passive: no_orb_action,
        evoke: no_orb_action,
        rules: OrbRules::default(),
    }
}

fn dark() -> OrbDef {
    OrbDef {
        id: DARK_ORB,
        loc_key: LocKey::new("orb.dark"),
        passive: no_orb_action,
        evoke: no_orb_action,
        rules: OrbRules::default(),
    }
}

fn plasma() -> OrbDef {
    OrbDef {
        id: PLASMA_ORB,
        loc_key: LocKey::new("orb.plasma"),
        passive: no_orb_action,
        evoke: no_orb_action,
        rules: OrbRules::default(),
    }
}

pub fn resolve_orb_value(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    base_amount: i32,
    kind: crate::core::query::OrbValueKind,
) -> Decimal {
    let Some(instance) = ctx.state.orb(orb) else {
        return Decimal::from(base_amount);
    };
    let calc = OrbValueCalc {
        player: instance.owner,
        orb,
        base_amount: Decimal::from(base_amount),
        amount: Decimal::from(base_amount),
        kind,
    };
    crate::core::rules::RulePipeline::modify_orb_value(ctx.registry, ctx.state, calc)
        .0
        .amount
}

fn no_orb_action(
    _ctx: &RuleCtx<'_>,
    _orb: OrbInstanceId,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}

#[allow(dead_code)]
fn _trigger_name(trigger: OrbTrigger) -> &'static str {
    match trigger {
        OrbTrigger::AfterTurnStart => "after_turn_start",
        OrbTrigger::BeforeTurnEnd => "before_turn_end",
    }
}
