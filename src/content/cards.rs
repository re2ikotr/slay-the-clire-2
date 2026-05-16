use crate::core::effect::Effect;
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

pub fn no_card_effect(
    _ctx: &CardPlayCtx<'_>,
    _card: CardInstanceId,
    _target: Option<CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}
