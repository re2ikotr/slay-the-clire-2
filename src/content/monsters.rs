use rust_decimal::Decimal;

use crate::content::powers::STRENGTH;
use crate::core::effect::{DamageFlags, DamageKind, DamageOp, Effect, Source};
use crate::core::event::Event;
use crate::core::ids::{CreatureId, LocKey, MonsterId};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc,
};
use crate::core::rules::RuleCtx;

pub type MonsterIntentFn = for<'a> fn(&RuleCtx<'a>, CreatureId) -> MonsterIntent;
pub type MonsterActFn = for<'a> fn(&RuleCtx<'a>, CreatureId) -> Vec<Effect>;
pub type MonsterEventFn = for<'a> fn(&RuleCtx<'a>, CreatureId, &Event) -> Vec<Effect>;
pub type MonsterModifyDamageFn = for<'a> fn(&RuleCtx<'a>, CreatureId, DamageCalc) -> DamageCalc;
pub type MonsterModifyBlockFn = for<'a> fn(&RuleCtx<'a>, CreatureId, BlockCalc) -> BlockCalc;
pub type MonsterModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, CreatureId, ResourceCostCalc) -> ResourceCostCalc;
pub type MonsterModifyResultPileFn =
    for<'a> fn(&RuleCtx<'a>, CreatureId, CardPlayResultPileCalc) -> CardPlayResultPileCalc;
pub type MonsterDecisionFn = for<'a> fn(&RuleCtx<'a>, CreatureId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct MonsterDef {
    pub id: MonsterId,
    pub loc_key: LocKey,
    pub max_hp: i32,
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
    pub modify_card_play_result_pile: Option<MonsterModifyResultPileFn>,
    pub decide: Option<MonsterDecisionFn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonsterIntent {
    Attack { amount: i32 },
    AttackAndBlock { attack: i32, block: i32 },
    Buff,
    Block { amount: i32 },
    Debuff,
    Unknown,
}

pub const NIBBIT: MonsterId = MonsterId::new("NIBBIT");

pub fn nibbit() -> MonsterDef {
    MonsterDef {
        id: NIBBIT,
        loc_key: LocKey::new("monster.nibbit"),
        max_hp: 42,
        intent: nibbit_intent,
        act: nibbit_act,
        rules: MonsterRules::default(),
    }
}

fn nibbit_intent(ctx: &RuleCtx<'_>, monster: CreatureId) -> MonsterIntent {
    match nibbit_move_index(ctx, monster) {
        0 => MonsterIntent::Attack { amount: 12 },
        1 => MonsterIntent::AttackAndBlock {
            attack: 6,
            block: 5,
        },
        _ => MonsterIntent::Buff,
    }
}

fn nibbit_act(ctx: &RuleCtx<'_>, monster: CreatureId) -> Vec<Effect> {
    let Some(player) = ctx.state.player_creature_id() else {
        return Vec::new();
    };

    match nibbit_move_index(ctx, monster) {
        0 => vec![nibbit_damage(monster, player, 12)],
        1 => vec![
            nibbit_damage(monster, player, 6),
            Effect::GainBlock {
                target: monster,
                amount: Decimal::from(5),
                source: Some(Source::Creature(monster)),
            },
        ],
        _ => vec![Effect::ApplyPower {
            target: monster,
            power: STRENGTH,
            amount: Decimal::from(2),
            source: Some(Source::Creature(monster)),
        }],
    }
}

fn nibbit_move_index(ctx: &RuleCtx<'_>, monster: CreatureId) -> u32 {
    ctx.state
        .creature(monster)
        .map(|creature| creature.turns_taken % 3)
        .unwrap_or(0)
}

fn nibbit_damage(monster: CreatureId, player: CreatureId, amount: i32) -> Effect {
    Effect::DealDamage(DamageOp {
        source: Some(Source::Creature(monster)),
        dealer: Some(monster),
        target: player,
        base_amount: Decimal::from(amount),
        kind: DamageKind::Attack,
        flags: DamageFlags {
            ignores_block: false,
        },
    })
}
