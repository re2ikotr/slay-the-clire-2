use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, DamageCalc, Decision, DecisionQuery, HpLossCalc,
    OrbPassiveTriggerCountCalc, OrbValueCalc, PowerAmountCalc, ResourceCostCalc, SummonAmountCalc,
    UnblockedDamageTargetCalc,
};
use crate::core::rules::RuleCtx;
use crate::core::state::{CardCosts, GameState};
use crate::registry::{DefRegistry, StaticRegistry};

pub type CardPlayFn =
    for<'a> fn(&CardPlayCtx<'a>, CardInstanceId, Option<CreatureId>) -> Vec<Effect>;
pub type CardEventFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, &Event) -> Vec<Effect>;
pub type CardModifyDamageFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, DamageCalc) -> DamageCalc;
pub type CardModifyBlockFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, BlockCalc) -> BlockCalc;
pub type CardModifyHpLossFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, HpLossCalc) -> HpLossCalc;
pub type CardModifyUnblockedDamageTargetFn = for<'a> fn(
    &RuleCtx<'a>,
    CardInstanceId,
    UnblockedDamageTargetCalc,
) -> UnblockedDamageTargetCalc;
pub type CardModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, CardInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type CardModifyResultPileFn =
    for<'a> fn(&RuleCtx<'a>, CardInstanceId, CardPlayResultPileCalc) -> CardPlayResultPileCalc;
pub type CardDecisionFn = for<'a> fn(&RuleCtx<'a>, CardInstanceId, &DecisionQuery) -> Decision;
pub type CardModifyPowerAmountFn =
    for<'a> fn(&RuleCtx<'a>, CardInstanceId, PowerAmountCalc) -> PowerAmountCalc;
pub type CardModifyOrbPassiveCountFn = for<'a> fn(
    &RuleCtx<'a>,
    CardInstanceId,
    OrbPassiveTriggerCountCalc,
) -> OrbPassiveTriggerCountCalc;
pub type CardModifyOrbValueFn =
    for<'a> fn(&RuleCtx<'a>, CardInstanceId, OrbValueCalc) -> OrbValueCalc;
pub type CardModifySummonAmountFn =
    for<'a> fn(&RuleCtx<'a>, CardInstanceId, SummonAmountCalc) -> SummonAmountCalc;

#[derive(Clone)]
pub struct CardDef {
    pub id: CardId,
    pub loc_key: LocKey,
    pub pool: CardPoolId,
    pub card_type: CardType,
    pub rarity: CardRarity,
    pub target: TargetType,
    pub base_costs: CardCosts,
    pub upgraded_costs: Option<CardCosts>,
    pub keywords: &'static [CardKeyword],
    pub upgraded_keywords: &'static [CardKeyword],
    pub tags: &'static [CardTag],
    pub can_generate_in_combat: bool,
    pub play: CardPlayFn,
    pub rules: CardRules,
}

impl CardDef {
    pub fn costs_for(&self, upgraded: bool) -> CardCosts {
        if upgraded {
            self.upgraded_costs.unwrap_or(self.base_costs)
        } else {
            self.base_costs
        }
    }

    pub fn has_keyword(&self, upgraded: bool, keyword: CardKeyword) -> bool {
        self.keywords.contains(&keyword) || (upgraded && self.upgraded_keywords.contains(&keyword))
    }

    pub fn has_tag(&self, tag: CardTag) -> bool {
        self.tags.contains(&tag)
    }
}

#[derive(Clone, Default)]
pub struct CardRules {
    pub on_event: Option<CardEventFn>,
    pub modify_damage_additive: Option<CardModifyDamageFn>,
    pub modify_damage_multiplicative: Option<CardModifyDamageFn>,
    pub modify_damage_cap: Option<CardModifyDamageFn>,
    pub modify_block_additive: Option<CardModifyBlockFn>,
    pub modify_block_multiplicative: Option<CardModifyBlockFn>,
    pub modify_hp_loss: Option<CardModifyHpLossFn>,
    pub modify_unblocked_damage_target: Option<CardModifyUnblockedDamageTargetFn>,
    pub modify_resource_cost: Option<CardModifyResourceCostFn>,
    pub modify_card_play_result_pile: Option<CardModifyResultPileFn>,
    pub modify_power_amount: Option<CardModifyPowerAmountFn>,
    pub modify_orb_passive_trigger_count: Option<CardModifyOrbPassiveCountFn>,
    pub modify_orb_value: Option<CardModifyOrbValueFn>,
    pub modify_summon_amount: Option<CardModifySummonAmountFn>,
    pub decide: Option<CardDecisionFn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardPoolId {
    Ironclad,
    Silent,
    Regent,
    Necrobinder,
    Defect,
    Colorless,
    Curse,
    Deprecated,
    Event,
    Quest,
    Status,
    Token,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardType {
    Attack,
    Skill,
    Power,
    Status,
    Curse,
    Quest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardRarity {
    Basic,
    Common,
    Uncommon,
    Rare,
    Ancient,
    Event,
    Token,
    Status,
    Curse,
    Quest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetType {
    None,
    Enemy,
    AllEnemies,
    RandomEnemy,
    SelfTarget,
    AnyPlayer,
    AnyAlly,
    AllAllies,
    AnyCreature,
    TargetedNoCreature,
    Osty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKeyword {
    Exhaust,
    Innate,
    Unplayable,
    Ethereal,
    Temporary,
    PurgeOnUse,
    FreeThisTurn,
    Retain,
    Sly,
    Eternal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTag {
    Strike,
    Defend,
    Minion,
    OstyAttack,
    Shiv,
}

pub struct CardPlayCtx<'a> {
    pub state: &'a GameState,
    pub registry: &'a StaticRegistry,
    pub paid_energy: i32,
    pub paid_stars: i32,
}

pub mod ironclad;

pub mod colorless;
pub mod curses;
pub mod defect;
pub mod events;
pub mod necrobinder;
pub mod quests;
pub mod regent;
pub mod silent;
pub mod statuses;
pub mod tokens;

pub use ironclad::*;

pub fn register_all_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    register_ironclad_cards(registry);
    colorless::register_colorless_cards(registry);
    curses::register_curse_cards(registry);
    defect::register_defect_cards(registry);
    events::register_event_cards(registry);
    necrobinder::register_necrobinder_cards(registry);
    quests::register_quest_cards(registry);
    regent::register_regent_cards(registry);
    silent::register_silent_cards(registry);
    statuses::register_status_cards(registry);
    tokens::register_token_cards(registry);
}
