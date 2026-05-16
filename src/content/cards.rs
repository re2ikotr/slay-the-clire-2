use rust_decimal::Decimal;

use crate::core::effect::{DamageFlags, DamageKind, DamageOp, Effect, Source};
use crate::core::event::Event;
use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey};
use crate::core::query::{BlockCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc};
use crate::core::rules::RuleCtx;
use crate::core::state::{CardCosts, GameState};

pub type CardPlayFn =
    for<'a> fn(&CardPlayCtx<'a>, CardInstanceId, Option<CreatureId>) -> Vec<Effect>;
pub type CardEventFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, &Event) -> Vec<Effect>;
pub type CardModifyDamageFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, DamageCalc) -> DamageCalc;
pub type CardModifyBlockFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, BlockCalc) -> BlockCalc;
pub type CardModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, CardInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type CardDecisionFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct CardDef {
    pub id: CardId,
    pub loc_key: LocKey,
    pub card_type: CardType,
    pub rarity: CardRarity,
    pub target: TargetType,
    pub base_costs: CardCosts,
    pub play: CardPlayFn,
    pub rules: CardRules,
}

#[derive(Clone, Default)]
pub struct CardRules {
    pub on_event: Option<CardEventFn>,
    pub modify_damage_additive: Option<CardModifyDamageFn>,
    pub modify_damage_multiplicative: Option<CardModifyDamageFn>,
    pub modify_damage_cap: Option<CardModifyDamageFn>,
    pub modify_block_additive: Option<CardModifyBlockFn>,
    pub modify_block_multiplicative: Option<CardModifyBlockFn>,
    pub modify_resource_cost: Option<CardModifyResourceCostFn>,
    pub decide: Option<CardDecisionFn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardType {
    Attack,
    Skill,
    Power,
    Status,
    Curse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardRarity {
    Basic,
    Common,
    Uncommon,
    Rare,
    Special,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetType {
    None,
    Enemy,
    AllEnemies,
    SelfTarget,
    AnyCreature,
}

pub struct CardPlayCtx<'a> {
    pub state: &'a GameState,
}

pub const STRIKE_IRONCLAD: CardId = CardId::new("STRIKE_IRONCLAD");
pub const DEFEND_IRONCLAD: CardId = CardId::new("DEFEND_IRONCLAD");

pub fn no_card_effect(
    _ctx: &CardPlayCtx<'_>,
    _card: CardInstanceId,
    _target: Option<CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}

pub fn strike_ironclad() -> CardDef {
    CardDef {
        id: STRIKE_IRONCLAD,
        loc_key: LocKey::new("card.strike_ironclad"),
        card_type: CardType::Attack,
        rarity: CardRarity::Basic,
        target: TargetType::Enemy,
        base_costs: CardCosts::energy(1),
        play: strike_ironclad_play,
        rules: CardRules::default(),
    }
}

pub fn defend_ironclad() -> CardDef {
    CardDef {
        id: DEFEND_IRONCLAD,
        loc_key: LocKey::new("card.defend_ironclad"),
        card_type: CardType::Skill,
        rarity: CardRarity::Basic,
        target: TargetType::SelfTarget,
        base_costs: CardCosts::energy(1),
        play: defend_ironclad_play,
        rules: CardRules::default(),
    }
}

fn strike_ironclad_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };

    vec![Effect::DealDamage(DamageOp {
        source: Some(Source::Card(card)),
        dealer: ctx.state.player_creature_id(),
        target,
        base_amount: Decimal::from(6),
        kind: DamageKind::Attack,
        flags: DamageFlags {
            ignores_block: false,
            is_attack: true,
        },
    })]
}

fn defend_ironclad_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = ctx.state.player_creature_id() else {
        return Vec::new();
    };

    vec![Effect::GainBlock {
        target,
        amount: Decimal::from(5),
        source: Some(Source::Card(card)),
    }]
}
