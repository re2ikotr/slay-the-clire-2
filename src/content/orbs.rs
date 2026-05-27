use rust_decimal::Decimal;

use crate::core::effect::{
    DamageFlags, DamageKind, DamageOp, Effect, OrbTrigger, RandomDamageOp, Source,
};
use crate::core::ids::{LocKey, OrbId, OrbInstanceId};
use crate::core::query::{
    OrbPassiveTriggerCountCalc, OrbValueCalc, PowerAmountCalc, SummonAmountCalc,
};
use crate::core::rules::RuleCtx;
use crate::registry::DefRegistry;

pub type OrbActionFn = for<'a> fn(
    &RuleCtx<'a>,
    OrbInstanceId,
    OrbTrigger,
    Option<crate::core::ids::CreatureId>,
) -> Vec<Effect>;
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
        passive: lightning_passive,
        evoke: lightning_evoke,
        rules: OrbRules::default(),
    }
}

fn frost() -> OrbDef {
    OrbDef {
        id: FROST_ORB,
        loc_key: LocKey::new("orb.frost"),
        passive: frost_passive,
        evoke: frost_evoke,
        rules: OrbRules::default(),
    }
}

fn dark() -> OrbDef {
    OrbDef {
        id: DARK_ORB,
        loc_key: LocKey::new("orb.dark"),
        passive: dark_passive,
        evoke: dark_evoke,
        rules: OrbRules::default(),
    }
}

fn plasma() -> OrbDef {
    OrbDef {
        id: PLASMA_ORB,
        loc_key: LocKey::new("orb.plasma"),
        passive: plasma_passive,
        evoke: plasma_evoke,
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

fn lightning_passive(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    trigger: OrbTrigger,
    target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    if trigger != OrbTrigger::BeforeTurnEnd && target.is_none() {
        return Vec::new();
    }
    lightning_damage(
        ctx,
        orb,
        target,
        3,
        crate::core::query::OrbValueKind::Passive,
    )
}

fn lightning_evoke(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    _trigger: OrbTrigger,
    target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    lightning_damage(ctx, orb, target, 8, crate::core::query::OrbValueKind::Evoke)
}

fn lightning_damage(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    target: Option<crate::core::ids::CreatureId>,
    base: i32,
    kind: crate::core::query::OrbValueKind,
) -> Vec<Effect> {
    let amount = resolve_orb_value(ctx, orb, base, kind);
    if let Some(target) = target {
        vec![Effect::DealDamage(DamageOp {
            source: Some(Source::System),
            dealer: ctx.state.orb_owner_creature(orb),
            target,
            base_amount: amount,
            kind: DamageKind::Power,
            flags: DamageFlags {
                ignores_block: false,
            },
        })]
    } else {
        vec![Effect::DealDamageToRandomEnemy(RandomDamageOp {
            source: Some(Source::System),
            dealer: ctx.state.orb_owner_creature(orb),
            base_amount: amount,
            kind: DamageKind::Power,
            flags: DamageFlags {
                ignores_block: false,
            },
            hit_count: 1,
        })]
    }
}

fn frost_passive(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    trigger: OrbTrigger,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    if trigger != OrbTrigger::BeforeTurnEnd {
        return Vec::new();
    }
    frost_block(ctx, orb, 2, crate::core::query::OrbValueKind::Passive)
}

fn frost_evoke(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    _trigger: OrbTrigger,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    frost_block(ctx, orb, 5, crate::core::query::OrbValueKind::Evoke)
}

fn frost_block(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    base: i32,
    kind: crate::core::query::OrbValueKind,
) -> Vec<Effect> {
    ctx.state
        .orb_owner_creature(orb)
        .map(|target| {
            vec![Effect::GainBlock {
                target,
                amount: resolve_orb_value(ctx, orb, base, kind),
                source: Some(Source::System),
            }]
        })
        .unwrap_or_default()
}

fn dark_passive(
    _ctx: &RuleCtx<'_>,
    _orb: OrbInstanceId,
    _trigger: OrbTrigger,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}

fn dark_evoke(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    _trigger: OrbTrigger,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    let Some(target) = ctx.state.alive_monster_ids().into_iter().min_by_key(|id| {
        ctx.state
            .creature(*id)
            .map(|creature| creature.hp)
            .unwrap_or(0)
    }) else {
        return Vec::new();
    };
    vec![Effect::DealDamage(DamageOp {
        source: Some(Source::System),
        dealer: ctx.state.orb_owner_creature(orb),
        target,
        base_amount: resolve_orb_value(ctx, orb, 6, crate::core::query::OrbValueKind::Evoke),
        kind: DamageKind::Power,
        flags: DamageFlags {
            ignores_block: false,
        },
    })]
}

fn plasma_passive(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    trigger: OrbTrigger,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    if trigger != OrbTrigger::AfterTurnStart {
        return Vec::new();
    }
    plasma_energy(ctx, orb, 1)
}

fn plasma_evoke(
    ctx: &RuleCtx<'_>,
    orb: OrbInstanceId,
    _trigger: OrbTrigger,
    _target: Option<crate::core::ids::CreatureId>,
) -> Vec<Effect> {
    plasma_energy(ctx, orb, 2)
}

fn plasma_energy(ctx: &RuleCtx<'_>, orb: OrbInstanceId, amount: i32) -> Vec<Effect> {
    ctx.state
        .orb(orb)
        .map(|instance| {
            vec![Effect::GainResource {
                player: instance.owner,
                resource: crate::core::state::ResourceKind::Energy,
                amount,
            }]
        })
        .unwrap_or_default()
}

#[allow(dead_code)]
fn _trigger_name(trigger: OrbTrigger) -> &'static str {
    match trigger {
        OrbTrigger::AfterTurnStart => "after_turn_start",
        OrbTrigger::BeforeTurnEnd => "before_turn_end",
    }
}
