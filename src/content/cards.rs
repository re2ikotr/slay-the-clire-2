use rust_decimal::Decimal;

use crate::content::powers::*;
use crate::core::effect::{
    CardFilter, DamageAllEnemiesOp, DamageFlags, DamageKind, DamageOp, Effect, MoveReason,
    RandomDamageOp, Source, UpgradeMode,
};
use crate::core::event::Event;
use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey};
use crate::core::query::{BlockCalc, DamageCalc, Decision, DecisionQuery, ResourceCostCalc};
use crate::core::rules::RuleCtx;
use crate::core::state::{CardCosts, GameState, PileId, PileKind, ResourceKind};
use crate::registry::{DefRegistry, StaticRegistry};

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
    Ancient,
    Special,
    Token,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetType {
    None,
    Enemy,
    AllEnemies,
    RandomEnemy,
    SelfTarget,
    AnyAlly,
    AnyCreature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKeyword {
    Exhaust,
    Innate,
    Unplayable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTag {
    Strike,
    Defend,
}

pub struct CardPlayCtx<'a> {
    pub state: &'a GameState,
    pub registry: &'a StaticRegistry,
    pub paid_energy: i32,
    pub paid_stars: i32,
}

#[derive(Clone, Copy)]
struct CardSpec {
    id: CardId,
    loc_key: LocKey,
    card_type: CardType,
    rarity: CardRarity,
    target: TargetType,
    base_costs: CardCosts,
    upgraded_costs: Option<CardCosts>,
    keywords: &'static [CardKeyword],
    upgraded_keywords: &'static [CardKeyword],
    tags: &'static [CardTag],
    can_generate_in_combat: bool,
    play: CardPlayFn,
}

impl CardSpec {
    fn def(self) -> CardDef {
        CardDef {
            id: self.id,
            loc_key: self.loc_key,
            card_type: self.card_type,
            rarity: self.rarity,
            target: self.target,
            base_costs: self.base_costs,
            upgraded_costs: self.upgraded_costs,
            keywords: self.keywords,
            upgraded_keywords: self.upgraded_keywords,
            tags: self.tags,
            can_generate_in_combat: self.can_generate_in_combat,
            play: self.play,
            rules: CardRules::default(),
        }
    }
}

const KW_NONE: &[CardKeyword] = &[];
const KW_EXHAUST: &[CardKeyword] = &[CardKeyword::Exhaust];
const KW_INNATE: &[CardKeyword] = &[CardKeyword::Innate];
const TAG_NONE: &[CardTag] = &[];
const TAG_STRIKE: &[CardTag] = &[CardTag::Strike];
const TAG_DEFEND: &[CardTag] = &[CardTag::Defend];

macro_rules! card_ids {
    ($($name:ident => $id:literal,)*) => {
        $(pub const $name: CardId = CardId::new($id);)*
    };
}

card_ids! {
    AGGRESSION => "AGGRESSION",
    ANGER => "ANGER",
    ARMAMENTS => "ARMAMENTS",
    ASHEN_STRIKE => "ASHEN_STRIKE",
    BARRICADE => "BARRICADE",
    BASH => "BASH",
    BATTLE_TRANCE => "BATTLE_TRANCE",
    BLOOD_WALL => "BLOOD_WALL",
    BLOODLETTING => "BLOODLETTING",
    BLUDGEON => "BLUDGEON",
    BODY_SLAM => "BODY_SLAM",
    BRAND => "BRAND",
    BREAK => "BREAK",
    BREAKTHROUGH => "BREAKTHROUGH",
    BULLY => "BULLY",
    BURNING_PACT => "BURNING_PACT",
    CASCADE => "CASCADE",
    CINDER => "CINDER",
    COLOSSUS => "COLOSSUS",
    CONFLAGRATION => "CONFLAGRATION",
    CORRUPTION => "CORRUPTION",
    CRIMSON_MANTLE => "CRIMSON_MANTLE",
    CRUELTY => "CRUELTY",
    DARK_EMBRACE => "DARK_EMBRACE",
    DEFEND_IRONCLAD => "DEFEND_IRONCLAD",
    DEMON_FORM => "DEMON_FORM",
    DEMONIC_SHIELD => "DEMONIC_SHIELD",
    DISMANTLE => "DISMANTLE",
    DOMINATE => "DOMINATE",
    DRUM_OF_BATTLE => "DRUM_OF_BATTLE",
    EVIL_EYE => "EVIL_EYE",
    EXPECT_A_FIGHT => "EXPECT_A_FIGHT",
    FEED => "FEED",
    FEEL_NO_PAIN => "FEEL_NO_PAIN",
    FIEND_FIRE => "FIEND_FIRE",
    FIGHT_ME => "FIGHT_ME",
    FLAME_BARRIER => "FLAME_BARRIER",
    FORGOTTEN_RITUAL => "FORGOTTEN_RITUAL",
    HAVOC => "HAVOC",
    HEADBUTT => "HEADBUTT",
    HELLRAISER => "HELLRAISER",
    HEMOKINESIS => "HEMOKINESIS",
    HOWL_FROM_BEYOND => "HOWL_FROM_BEYOND",
    IMPERVIOUS => "IMPERVIOUS",
    INFERNAL_BLADE => "INFERNAL_BLADE",
    INFERNO => "INFERNO",
    INFLAME => "INFLAME",
    IRON_WAVE => "IRON_WAVE",
    JUGGERNAUT => "JUGGERNAUT",
    JUGGLING => "JUGGLING",
    MANGLE => "MANGLE",
    MOLTEN_FIST => "MOLTEN_FIST",
    NOT_YET => "NOT_YET",
    OFFERING => "OFFERING",
    ONE_TWO_PUNCH => "ONE_TWO_PUNCH",
    PACTS_END => "PACTS_END",
    PERFECTED_STRIKE => "PERFECTED_STRIKE",
    PILLAGE => "PILLAGE",
    POMMEL_STRIKE => "POMMEL_STRIKE",
    PRIMAL_FORCE => "PRIMAL_FORCE",
    PYRE => "PYRE",
    RAGE => "RAGE",
    RAMPAGE => "RAMPAGE",
    RUPTURE => "RUPTURE",
    SECOND_WIND => "SECOND_WIND",
    SETUP_STRIKE => "SETUP_STRIKE",
    SHRUG_IT_OFF => "SHRUG_IT_OFF",
    SPITE => "SPITE",
    STAMPEDE => "STAMPEDE",
    STOKE => "STOKE",
    STOMP => "STOMP",
    STONE_ARMOR => "STONE_ARMOR",
    STRIKE_IRONCLAD => "STRIKE_IRONCLAD",
    SWORD_BOOMERANG => "SWORD_BOOMERANG",
    TANK => "TANK",
    TAUNT => "TAUNT",
    TEAR_ASUNDER => "TEAR_ASUNDER",
    THRASH => "THRASH",
    THUNDERCLAP => "THUNDERCLAP",
    TREMBLE => "TREMBLE",
    TRUE_GRIT => "TRUE_GRIT",
    TWIN_STRIKE => "TWIN_STRIKE",
    UNMOVABLE => "UNMOVABLE",
    UNRELENTING => "UNRELENTING",
    UPPERCUT => "UPPERCUT",
    VICIOUS => "VICIOUS",
    WHIRLWIND => "WHIRLWIND",
    GIANT_ROCK => "GIANT_ROCK",
}

macro_rules! spec {
    ($id:ident, $type:ident, $rarity:ident, $target:ident, $cost:expr, $upcost:expr, $kw:expr, $upkw:expr, $tags:expr, $gen:expr, $play:ident) => {
        CardSpec {
            id: $id,
            loc_key: LocKey::new(concat!("card.", stringify!($id))),
            card_type: CardType::$type,
            rarity: CardRarity::$rarity,
            target: TargetType::$target,
            base_costs: $cost,
            upgraded_costs: $upcost,
            keywords: $kw,
            upgraded_keywords: $upkw,
            tags: $tags,
            can_generate_in_combat: $gen,
            play: $play,
        }
    };
}

const IRONCLAD_CARD_SPECS: &[CardSpec] = &[
    spec!(
        AGGRESSION,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_INNATE,
        TAG_NONE,
        true,
        aggression_play
    ),
    spec!(
        ANGER,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        anger_play
    ),
    spec!(
        ARMAMENTS,
        Skill,
        Common,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        armaments_play
    ),
    spec!(
        ASHEN_STRIKE,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_STRIKE,
        true,
        ashen_strike_play
    ),
    spec!(
        BARRICADE,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(3),
        Some(CardCosts::energy(2)),
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        barricade_play
    ),
    spec!(
        BASH,
        Attack,
        Basic,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        bash_play
    ),
    spec!(
        BATTLE_TRANCE,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        battle_trance_play
    ),
    spec!(
        BLOOD_WALL,
        Skill,
        Common,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        blood_wall_play
    ),
    spec!(
        BLOODLETTING,
        Skill,
        Common,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        bloodletting_play
    ),
    spec!(
        BLUDGEON,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(3),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        bludgeon_play
    ),
    spec!(
        BODY_SLAM,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        Some(CardCosts::energy(0)),
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        body_slam_play
    ),
    spec!(
        BRAND,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        brand_play
    ),
    spec!(
        BREAK,
        Attack,
        Ancient,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        break_play
    ),
    spec!(
        BREAKTHROUGH,
        Attack,
        Common,
        AllEnemies,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        breakthrough_play
    ),
    spec!(
        BULLY,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        bully_play
    ),
    spec!(
        BURNING_PACT,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        burning_pact_play
    ),
    spec!(
        CASCADE,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::x_energy(),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        cascade_play
    ),
    spec!(
        CINDER,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        cinder_play
    ),
    spec!(
        COLOSSUS,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        colossus_play
    ),
    spec!(
        CONFLAGRATION,
        Attack,
        Rare,
        AllEnemies,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        conflagration_play
    ),
    spec!(
        CORRUPTION,
        Power,
        Ancient,
        SelfTarget,
        CardCosts::energy(3),
        Some(CardCosts::energy(2)),
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        corruption_play
    ),
    spec!(
        CRIMSON_MANTLE,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        crimson_mantle_play
    ),
    spec!(
        CRUELTY,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        cruelty_play
    ),
    spec!(
        DARK_EMBRACE,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        Some(CardCosts::energy(1)),
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        dark_embrace_play
    ),
    spec!(
        DEFEND_IRONCLAD,
        Skill,
        Basic,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_DEFEND,
        true,
        defend_ironclad_play
    ),
    spec!(
        DEMON_FORM,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(3),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        demon_form_play
    ),
    spec!(
        DEMONIC_SHIELD,
        Skill,
        Uncommon,
        AnyAlly,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        &[],
        TAG_NONE,
        true,
        demonic_shield_play
    ),
    spec!(
        DISMANTLE,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        dismantle_play
    ),
    spec!(
        DOMINATE,
        Skill,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        dominate_play
    ),
    spec!(
        DRUM_OF_BATTLE,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        drum_of_battle_play
    ),
    spec!(
        EVIL_EYE,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        evil_eye_play
    ),
    spec!(
        EXPECT_A_FIGHT,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(2),
        Some(CardCosts::energy(1)),
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        expect_a_fight_play
    ),
    spec!(
        FEED,
        Attack,
        Rare,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        feed_play
    ),
    spec!(
        FEEL_NO_PAIN,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        feel_no_pain_play
    ),
    spec!(
        FIEND_FIRE,
        Attack,
        Rare,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        fiend_fire_play
    ),
    spec!(
        FIGHT_ME,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        fight_me_play
    ),
    spec!(
        FLAME_BARRIER,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        flame_barrier_play
    ),
    spec!(
        FORGOTTEN_RITUAL,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        forgotten_ritual_play
    ),
    spec!(
        HAVOC,
        Skill,
        Common,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        havoc_play
    ),
    spec!(
        HEADBUTT,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        headbutt_play
    ),
    spec!(
        HELLRAISER,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        hellraiser_play
    ),
    spec!(
        HEMOKINESIS,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        hemokinesis_play
    ),
    spec!(
        HOWL_FROM_BEYOND,
        Attack,
        Uncommon,
        AllEnemies,
        CardCosts::energy(3),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        howl_from_beyond_play
    ),
    spec!(
        IMPERVIOUS,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        impervious_play
    ),
    spec!(
        INFERNAL_BLADE,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        infernal_blade_play
    ),
    spec!(
        INFERNO,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        inferno_play
    ),
    spec!(
        INFLAME,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        inflame_play
    ),
    spec!(
        IRON_WAVE,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        iron_wave_play
    ),
    spec!(
        JUGGERNAUT,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        juggernaut_play
    ),
    spec!(
        JUGGLING,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_INNATE,
        TAG_NONE,
        true,
        juggling_play
    ),
    spec!(
        MANGLE,
        Attack,
        Rare,
        Enemy,
        CardCosts::energy(3),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        mangle_play
    ),
    spec!(
        MOLTEN_FIST,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        molten_fist_play
    ),
    spec!(
        NOT_YET,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        false,
        not_yet_play
    ),
    spec!(
        OFFERING,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        offering_play
    ),
    spec!(
        ONE_TWO_PUNCH,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        one_two_punch_play
    ),
    spec!(
        PACTS_END,
        Attack,
        Rare,
        AllEnemies,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        pacts_end_play
    ),
    spec!(
        PERFECTED_STRIKE,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_STRIKE,
        true,
        perfected_strike_play
    ),
    spec!(
        PILLAGE,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        pillage_play
    ),
    spec!(
        POMMEL_STRIKE,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_STRIKE,
        true,
        pommel_strike_play
    ),
    spec!(
        PRIMAL_FORCE,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        primal_force_play
    ),
    spec!(
        PYRE,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        pyre_play
    ),
    spec!(
        RAGE,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        rage_play
    ),
    spec!(
        RAMPAGE,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        rampage_play
    ),
    spec!(
        RUPTURE,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        rupture_play
    ),
    spec!(
        SECOND_WIND,
        Skill,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        second_wind_play
    ),
    spec!(
        SETUP_STRIKE,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_STRIKE,
        true,
        setup_strike_play
    ),
    spec!(
        SHRUG_IT_OFF,
        Skill,
        Common,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        shrug_it_off_play
    ),
    spec!(
        SPITE,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(0),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        spite_play
    ),
    spec!(
        STAMPEDE,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(2),
        Some(CardCosts::energy(1)),
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        stampede_play
    ),
    spec!(
        STOKE,
        Skill,
        Rare,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        stoke_play
    ),
    spec!(
        STOMP,
        Attack,
        Uncommon,
        AllEnemies,
        CardCosts::energy(3),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        stomp_play
    ),
    spec!(
        STONE_ARMOR,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        stone_armor_play
    ),
    spec!(
        STRIKE_IRONCLAD,
        Attack,
        Basic,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_STRIKE,
        true,
        strike_ironclad_play
    ),
    spec!(
        SWORD_BOOMERANG,
        Attack,
        Common,
        RandomEnemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        sword_boomerang_play
    ),
    spec!(
        TANK,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(1),
        Some(CardCosts::energy(0)),
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        tank_play
    ),
    spec!(
        TAUNT,
        Skill,
        Uncommon,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        taunt_play
    ),
    spec!(
        TEAR_ASUNDER,
        Attack,
        Rare,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        tear_asunder_play
    ),
    spec!(
        THRASH,
        Attack,
        Rare,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        thrash_play
    ),
    spec!(
        THUNDERCLAP,
        Attack,
        Common,
        AllEnemies,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        thunderclap_play
    ),
    spec!(
        TREMBLE,
        Skill,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        tremble_play
    ),
    spec!(
        TRUE_GRIT,
        Skill,
        Common,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        true_grit_play
    ),
    spec!(
        TWIN_STRIKE,
        Attack,
        Common,
        Enemy,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_STRIKE,
        true,
        twin_strike_play
    ),
    spec!(
        UNMOVABLE,
        Power,
        Rare,
        SelfTarget,
        CardCosts::energy(2),
        Some(CardCosts::energy(1)),
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        unmovable_play
    ),
    spec!(
        UNRELENTING,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        unrelenting_play
    ),
    spec!(
        UPPERCUT,
        Attack,
        Uncommon,
        Enemy,
        CardCosts::energy(2),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        uppercut_play
    ),
    spec!(
        VICIOUS,
        Power,
        Uncommon,
        SelfTarget,
        CardCosts::energy(1),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        vicious_play
    ),
    spec!(
        WHIRLWIND,
        Attack,
        Uncommon,
        AllEnemies,
        CardCosts::x_energy(),
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        true,
        whirlwind_play
    ),
];

const GIANT_ROCK_SPEC: CardSpec = spec!(
    GIANT_ROCK,
    Attack,
    Token,
    Enemy,
    CardCosts::energy(1),
    None,
    KW_NONE,
    KW_NONE,
    TAG_NONE,
    true,
    giant_rock_play
);

pub fn register_ironclad_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    for spec in IRONCLAD_CARD_SPECS {
        registry.register(spec.def());
    }
    registry.register(GIANT_ROCK_SPEC.def());
}

pub fn ironclad_card_defs() -> Vec<CardDef> {
    IRONCLAD_CARD_SPECS.iter().map(|spec| spec.def()).collect()
}

pub fn strike_ironclad() -> CardDef {
    IRONCLAD_CARD_SPECS
        .iter()
        .find(|spec| spec.id == STRIKE_IRONCLAD)
        .expect("strike spec exists")
        .def()
}

pub fn defend_ironclad() -> CardDef {
    IRONCLAD_CARD_SPECS
        .iter()
        .find(|spec| spec.id == DEFEND_IRONCLAD)
        .expect("defend spec exists")
        .def()
}

pub fn no_card_effect(
    _ctx: &CardPlayCtx<'_>,
    _card: CardInstanceId,
    _target: Option<CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}

macro_rules! simple_attack {
    ($fn_name:ident, $base:expr, $upgrade_delta:expr) => {
        fn $fn_name(
            ctx: &CardPlayCtx<'_>,
            card: CardInstanceId,
            target: Option<CreatureId>,
        ) -> Vec<Effect> {
            target
                .map(|target| {
                    attack_effects(
                        ctx,
                        card,
                        target,
                        value(ctx, card, $base, $upgrade_delta),
                        1,
                    )
                })
                .unwrap_or_default()
        }
    };
}

simple_attack!(strike_ironclad_play, 6, 3);
simple_attack!(bludgeon_play, 32, 10);
simple_attack!(giant_rock_play, 16, 4);

fn aggression_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, AGGRESSION_POWER, 1)
}

fn anger_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = strike_like(ctx, card, target, 6, 2, 1);
    if let Some(player) = ctx.state.player_id() {
        effects.push(Effect::AddGeneratedCard {
            player,
            def: ANGER,
            to: PileId::player(player, PileKind::Discard),
            upgraded: is_upgraded(ctx, card),
            temporary: false,
            zero_cost_this_turn: false,
        });
    }
    effects
}

fn armaments_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = block_self(ctx, card, 5, 0);
    effects.push(Effect::UpgradeHand {
        player,
        mode: if is_upgraded(ctx, card) {
            UpgradeMode::All
        } else {
            UpgradeMode::First
        },
    });
    effects
}

fn ashen_strike_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let exhaust_count = ctx
        .state
        .combat()
        .map(|combat| combat.player.piles.exhaust.len())
        .unwrap_or(0) as i32;
    let extra = if is_upgraded(ctx, card) { 4 } else { 3 };
    target
        .map(|target| attack_effects(ctx, card, target, 6 + extra * exhaust_count, 1))
        .unwrap_or_default()
}

fn barricade_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, BARRICADE_POWER, 1)
}

fn bash_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let mut effects = attack_effects(ctx, card, target, value(ctx, card, 8, 2), 1);
    effects.push(apply_power(
        target,
        card,
        VULNERABLE,
        if is_upgraded(ctx, card) { 3 } else { 2 },
    ));
    effects
}

fn battle_trance_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = vec![Effect::DrawCards {
        player,
        count: value(ctx, card, 3, 1) as u8,
    }];
    effects.extend(apply_self_power(ctx, card, NO_DRAW_POWER, 1));
    effects
}

fn blood_wall_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = lose_self_hp(ctx, card, 2);
    effects.extend(block_self(ctx, card, 16, 4));
    effects
}

fn bloodletting_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = lose_self_hp(ctx, card, 3);
    effects.push(Effect::GainResource {
        player,
        resource: ResourceKind::Energy,
        amount: value(ctx, card, 2, 1),
    });
    effects
}

fn body_slam_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let block = ctx
        .state
        .player_creature_id()
        .and_then(|id| ctx.state.creature(id))
        .map(|creature| creature.block)
        .unwrap_or(0);
    attack_effects(ctx, card, target, block, 1)
}

fn brand_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = lose_self_hp(ctx, card, 1);
    effects.push(Effect::ExhaustRandomHand {
        player,
        filter: CardFilter::Any,
    });
    effects.extend(apply_self_power(
        ctx,
        card,
        STRENGTH,
        value(ctx, card, 1, 1),
    ));
    effects
}

fn break_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let mut effects = attack_effects(ctx, card, target, value(ctx, card, 20, 10), 1);
    effects.push(apply_power(
        target,
        card,
        VULNERABLE,
        if is_upgraded(ctx, card) { 7 } else { 5 },
    ));
    effects
}

fn breakthrough_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = lose_self_hp(ctx, card, 1);
    effects.push(all_enemy_attack(ctx, card, value(ctx, card, 9, 4), 1));
    effects
}

fn bully_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let vuln = ctx.state.power_amount(target, VULNERABLE);
    let per_vuln = if is_upgraded(ctx, card) { 3 } else { 2 };
    attack_effects(ctx, card, target, 4 + per_vuln * vuln, 1)
}

fn burning_pact_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    vec![
        Effect::ExhaustRandomHand {
            player,
            filter: CardFilter::Any,
        },
        Effect::DrawCards {
            player,
            count: value(ctx, card, 2, 1) as u8,
        },
    ]
}

fn cascade_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    vec![Effect::PlayTopDrawCards {
        player,
        count: (ctx.paid_energy + if is_upgraded(ctx, card) { 1 } else { 0 }).max(0) as u8,
        exhaust_after_play: false,
    }]
}

fn cinder_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = strike_like(ctx, card, target, 18, 6, 1);
    effects.push(Effect::ExhaustTopDraw { player, count: 1 });
    effects
}

fn colossus_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = block_self(ctx, card, 5, 3);
    effects.extend(apply_self_power(ctx, card, COLOSSUS_POWER, 1));
    effects
}

fn conflagration_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let attacks = ctx
        .state
        .combat()
        .map(|combat| combat.turn_stats.attacks_played.saturating_sub(1))
        .unwrap_or(0) as i32;
    let extra = if is_upgraded(ctx, card) { 3 } else { 2 };
    vec![all_enemy_attack(ctx, card, 8 + attacks * extra, 1)]
}

fn corruption_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, CORRUPTION_POWER, 1)
}

fn crimson_mantle_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, CRIMSON_MANTLE_POWER, value(ctx, card, 8, 2))
}

fn cruelty_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(
        ctx,
        card,
        CRUELTY_POWER,
        if is_upgraded(ctx, card) { 50 } else { 25 },
    )
}

fn dark_embrace_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, DARK_EMBRACE_POWER, 1)
}

fn defend_ironclad_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    block_self(ctx, card, 5, 3)
}

fn demon_form_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, DEMON_FORM_POWER, value(ctx, card, 2, 1))
}

fn demonic_shield_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let target = target.or_else(|| ctx.state.player_creature_id());
    let Some(target) = target else {
        return Vec::new();
    };
    let block = ctx
        .state
        .player_creature_id()
        .and_then(|id| ctx.state.creature(id))
        .map(|creature| creature.block)
        .unwrap_or(0);
    let mut effects = lose_self_hp(ctx, card, 1);
    effects.push(Effect::GainBlock {
        target,
        amount: Decimal::from(block),
        source: Some(Source::Card(card)),
    });
    effects
}

fn dismantle_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let hits = if ctx.state.power_amount(target, VULNERABLE) > 0 {
        2
    } else {
        1
    };
    attack_effects(ctx, card, target, value(ctx, card, 8, 2), hits)
}

fn dominate_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let mut effects = vec![apply_power(
        target,
        card,
        VULNERABLE,
        if is_upgraded(ctx, card) { 2 } else { 1 },
    )];
    let vuln =
        ctx.state.power_amount(target, VULNERABLE) + if is_upgraded(ctx, card) { 2 } else { 1 };
    effects.extend(apply_self_power(ctx, card, STRENGTH, vuln));
    effects
}

fn drum_of_battle_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = vec![Effect::DrawCards {
        player,
        count: value(ctx, card, 2, 1) as u8,
    }];
    effects.extend(apply_self_power(ctx, card, DRUM_OF_BATTLE_POWER, 1));
    effects
}

fn evil_eye_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let repeats = if ctx
        .state
        .combat()
        .map(|c| c.turn_stats.cards_exhausted > 0)
        .unwrap_or(false)
    {
        2
    } else {
        1
    };
    let mut effects = Vec::new();
    for _ in 0..repeats {
        effects.extend(block_self(ctx, card, 8, 3));
    }
    effects
}

fn expect_a_fight_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let attacks = count_hand_type(ctx, CardType::Attack);
    let mut effects = vec![Effect::GainResource {
        player,
        resource: ResourceKind::Energy,
        amount: attacks,
    }];
    effects.extend(apply_self_power(ctx, card, NO_ENERGY_GAIN_POWER, 1));
    effects
}

fn feed_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    // Fatal max HP is resolved optimistically after damage; CheckDeaths will settle combat state.
    let Some(target) = target else {
        return Vec::new();
    };
    let mut effects = attack_effects(ctx, card, target, value(ctx, card, 10, 2), 1);
    if ctx
        .state
        .creature(target)
        .map(|c| c.hp <= value(ctx, card, 10, 2))
        .unwrap_or(false)
    {
        if let Some(player) = ctx.state.player_creature_id() {
            effects.push(Effect::GainMaxHp {
                target: player,
                amount: value(ctx, card, 3, 1),
                source: Some(Source::Card(card)),
            });
        }
    }
    effects
}

fn feel_no_pain_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, FEEL_NO_PAIN_POWER, value(ctx, card, 3, 1))
}

fn fiend_fire_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let Some(target) = target else {
        return Vec::new();
    };
    let count = ctx
        .state
        .combat()
        .map(|c| c.player.piles.hand.len())
        .unwrap_or(0) as u8;
    let mut effects = vec![Effect::ExhaustHand {
        player,
        filter: CardFilter::Any,
    }];
    effects.extend(attack_effects(
        ctx,
        card,
        target,
        value(ctx, card, 7, 3),
        count,
    ));
    effects
}

fn fight_me_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let mut effects = attack_effects(ctx, card, target, value(ctx, card, 5, 1), 2);
    effects.extend(apply_self_power(
        ctx,
        card,
        STRENGTH,
        if is_upgraded(ctx, card) { 3 } else { 2 },
    ));
    effects.push(apply_power(target, card, STRENGTH, 1));
    effects
}

fn flame_barrier_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = block_self(ctx, card, 12, 4);
    effects.extend(apply_self_power(
        ctx,
        card,
        FLAME_BARRIER_POWER,
        value(ctx, card, 4, 2),
    ));
    effects
}

fn forgotten_ritual_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    if ctx
        .state
        .combat()
        .map(|c| c.turn_stats.cards_exhausted > 0)
        .unwrap_or(false)
    {
        vec![Effect::GainResource {
            player,
            resource: ResourceKind::Energy,
            amount: value(ctx, card, 3, 1),
        }]
    } else {
        Vec::new()
    }
}

fn havoc_play(ctx: &CardPlayCtx<'_>, _card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    ctx.state
        .player_id()
        .map(|player| {
            vec![Effect::PlayTopDrawCards {
                player,
                count: 1,
                exhaust_after_play: true,
            }]
        })
        .unwrap_or_default()
}

fn headbutt_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = strike_like(ctx, card, target, 9, 3, 1);
    if let Some(discard_top) = ctx
        .state
        .combat()
        .and_then(|c| c.player.piles.discard.last().copied())
    {
        effects.push(Effect::MoveCard {
            card: discard_top,
            to: PileId::player(player, PileKind::Draw),
            reason: MoveReason::Generated,
        });
    }
    effects
}

fn hellraiser_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, HELLRAISER_POWER, 1)
}

fn hemokinesis_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = lose_self_hp(ctx, card, 2);
    effects.extend(strike_like(ctx, card, target, 14, 5, 1));
    effects
}

fn howl_from_beyond_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    vec![all_enemy_attack(ctx, card, value(ctx, card, 16, 5), 1)]
}

fn impervious_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    block_self(ctx, card, 30, 10)
}

fn infernal_blade_play(
    ctx: &CardPlayCtx<'_>,
    _: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    ctx.state
        .player_id()
        .map(|player| {
            vec![Effect::GenerateRandomCardToHand {
                player,
                card_type: Some(CardType::Attack),
                target: None,
                zero_cost_this_turn: true,
            }]
        })
        .unwrap_or_default()
}

fn inferno_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, INFERNO_POWER, value(ctx, card, 6, 3))
}

fn inflame_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, STRENGTH, value(ctx, card, 2, 1))
}

fn iron_wave_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = block_self(ctx, card, 5, 2);
    effects.extend(strike_like(ctx, card, target, 5, 2, 1));
    effects
}

fn juggernaut_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, JUGGERNAUT_POWER, value(ctx, card, 5, 2))
}

fn juggling_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, JUGGLING_POWER, 1)
}

fn mangle_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let loss = value(ctx, card, 10, 5);
    let mut effects = attack_effects(ctx, card, target, value(ctx, card, 15, 5), 1);
    effects.push(apply_power(target, card, STRENGTH, -loss));
    effects.push(apply_power(target, card, MANGLE_POWER, loss));
    effects
}

fn molten_fist_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let vuln = ctx.state.power_amount(target, VULNERABLE);
    let mut effects = attack_effects(ctx, card, target, value(ctx, card, 10, 4), 1);
    if vuln > 0 {
        effects.push(apply_power(target, card, VULNERABLE, vuln));
    }
    effects
}

fn not_yet_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    ctx.state
        .player_creature_id()
        .map(|target| {
            vec![Effect::Heal {
                target,
                amount: Decimal::from(value(ctx, card, 10, 3)),
                source: Some(Source::Card(card)),
            }]
        })
        .unwrap_or_default()
}

fn offering_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = lose_self_hp(ctx, card, 6);
    effects.push(Effect::GainResource {
        player,
        resource: ResourceKind::Energy,
        amount: 2,
    });
    effects.push(Effect::DrawCards {
        player,
        count: value(ctx, card, 3, 2) as u8,
    });
    effects
}

fn one_two_punch_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, ONE_TWO_PUNCH_POWER, value(ctx, card, 1, 1))
}

fn pacts_end_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    vec![all_enemy_attack(ctx, card, value(ctx, card, 17, 6), 1)]
}

fn perfected_strike_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let strikes = count_all_tag(ctx, CardTag::Strike);
    let extra = if is_upgraded(ctx, card) { 3 } else { 2 };
    target
        .map(|target| attack_effects(ctx, card, target, 6 + strikes * extra, 1))
        .unwrap_or_default()
}

fn pillage_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = strike_like(ctx, card, target, 6, 3, 1);
    effects.push(Effect::DrawUntilNonAttack { player });
    effects
}

fn pommel_strike_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = strike_like(ctx, card, target, 9, 1, 1);
    effects.push(Effect::DrawCards {
        player,
        count: value(ctx, card, 1, 1) as u8,
    });
    effects
}

fn primal_force_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let attacks = hand_matching(ctx, CardFilter::Attack);
    let mut effects = Vec::new();
    for attack in attacks {
        effects.push(Effect::ExhaustCard { card: attack });
        effects.push(Effect::AddGeneratedCard {
            player,
            def: GIANT_ROCK,
            to: PileId::player(player, PileKind::Hand),
            upgraded: is_upgraded(ctx, card),
            temporary: true,
            zero_cost_this_turn: false,
        });
    }
    effects
}

fn pyre_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, PYRE_POWER, value(ctx, card, 1, 1))
}

fn rage_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, RAGE_POWER, value(ctx, card, 3, 2))
}

fn rampage_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    strike_like(ctx, card, target, 9, 0, 1)
}

fn rupture_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, RUPTURE_POWER, value(ctx, card, 1, 1))
}

fn second_wind_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let count = hand_matching(ctx, CardFilter::NonAttack).len() as u8;
    let mut effects = vec![Effect::ExhaustHand {
        player,
        filter: CardFilter::NonAttack,
    }];
    for _ in 0..count {
        effects.extend(block_self(ctx, card, 5, 2));
    }
    effects
}

fn setup_strike_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = strike_like(ctx, card, target, 7, 2, 1);
    effects.extend(apply_self_power(ctx, card, STRENGTH, 2));
    effects.extend(apply_self_power(ctx, card, SETUP_STRIKE_POWER, 2));
    effects
}

fn shrug_it_off_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = block_self(ctx, card, 8, 3);
    effects.push(Effect::DrawCards { player, count: 1 });
    effects
}

fn spite_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let hits = if ctx
        .state
        .combat()
        .map(|c| c.turn_stats.hp_lost_by_player > 0)
        .unwrap_or(false)
    {
        value(ctx, card, 2, 1) as u8
    } else {
        1
    };
    strike_like(ctx, card, target, 5, 0, hits)
}

fn stampede_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, STAMPEDE_POWER, 1)
}

fn stoke_play(ctx: &CardPlayCtx<'_>, _card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let count = ctx
        .state
        .combat()
        .map(|c| c.player.piles.hand.len())
        .unwrap_or(0) as u8;
    vec![
        Effect::ExhaustHand {
            player,
            filter: CardFilter::Any,
        },
        Effect::DrawCards { player, count },
    ]
}

fn stomp_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    vec![all_enemy_attack(ctx, card, value(ctx, card, 12, 3), 1)]
}

fn stone_armor_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, PLATING_POWER, value(ctx, card, 4, 2))
}

fn sword_boomerang_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    vec![random_enemy_attack(
        ctx,
        card,
        3,
        value(ctx, card, 3, 1) as u8,
    )]
}

fn tank_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, TANK_POWER, 1)
}

fn taunt_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let mut effects = block_self(ctx, card, 7, 1);
    effects.push(apply_power(
        target,
        card,
        VULNERABLE,
        if is_upgraded(ctx, card) { 2 } else { 1 },
    ));
    effects
}

fn tear_asunder_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let hits = 1 + ctx
        .state
        .combat()
        .map(|c| c.combat_stats.hp_loss_events_by_player)
        .unwrap_or(0) as u8;
    strike_like(ctx, card, target, 5, 2, hits)
}

fn thrash_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = strike_like(ctx, card, target, 4, 2, 2);
    effects.push(Effect::ExhaustRandomHand {
        player,
        filter: CardFilter::Attack,
    });
    effects
}

fn thunderclap_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = vec![all_enemy_attack(ctx, card, value(ctx, card, 4, 3), 1)];
    for enemy in ctx.state.alive_monster_ids() {
        effects.push(apply_power(enemy, card, VULNERABLE, 1));
    }
    effects
}

fn tremble_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    target
        .map(|target| {
            vec![apply_power(
                target,
                card,
                VULNERABLE,
                value(ctx, card, 3, 1),
            )]
        })
        .unwrap_or_default()
}

fn true_grit_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(player) = ctx.state.player_id() else {
        return Vec::new();
    };
    let mut effects = block_self(ctx, card, 7, 2);
    effects.push(Effect::ExhaustRandomHand {
        player,
        filter: CardFilter::Any,
    });
    effects
}

fn twin_strike_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    strike_like(ctx, card, target, 5, 2, 2)
}

fn unmovable_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    apply_self_power(ctx, card, UNMOVABLE_POWER, 1)
}

fn unrelenting_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = strike_like(ctx, card, target, 12, 6, 1);
    effects.extend(apply_self_power(ctx, card, FREE_ATTACK_POWER, 1));
    effects
}

fn uppercut_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let Some(target) = target else {
        return Vec::new();
    };
    let amount = if is_upgraded(ctx, card) { 2 } else { 1 };
    let mut effects = attack_effects(ctx, card, target, 13, 1);
    effects.push(apply_power(target, card, WEAK, amount));
    effects.push(apply_power(target, card, VULNERABLE, amount));
    effects
}

fn vicious_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    apply_self_power(ctx, card, VICIOUS_POWER, value(ctx, card, 1, 1))
}

fn whirlwind_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    vec![all_enemy_attack(
        ctx,
        card,
        value(ctx, card, 5, 3),
        ctx.paid_energy.max(0) as u8,
    )]
}

fn strike_like(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
    base: i32,
    upgrade_delta: i32,
    hits: u8,
) -> Vec<Effect> {
    target
        .map(|target| {
            attack_effects(
                ctx,
                card,
                target,
                value(ctx, card, base, upgrade_delta),
                hits,
            )
        })
        .unwrap_or_default()
}

fn attack_effects(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: CreatureId,
    amount: i32,
    hits: u8,
) -> Vec<Effect> {
    (0..hits)
        .map(|_| {
            Effect::DealDamage(DamageOp {
                source: Some(Source::Card(card)),
                dealer: ctx.state.player_creature_id(),
                target,
                base_amount: Decimal::from(amount),
                kind: DamageKind::Attack,
                flags: DamageFlags {
                    ignores_block: false,
                    is_attack: true,
                },
            })
        })
        .collect()
}

fn all_enemy_attack(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    amount: i32,
    hit_count: u8,
) -> Effect {
    Effect::DealDamageToAllEnemies(DamageAllEnemiesOp {
        source: Some(Source::Card(card)),
        dealer: ctx.state.player_creature_id(),
        base_amount: Decimal::from(amount),
        kind: DamageKind::Attack,
        flags: DamageFlags {
            ignores_block: false,
            is_attack: true,
        },
        hit_count,
    })
}

fn random_enemy_attack(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    amount: i32,
    hit_count: u8,
) -> Effect {
    Effect::DealDamageToRandomEnemy(RandomDamageOp {
        source: Some(Source::Card(card)),
        dealer: ctx.state.player_creature_id(),
        base_amount: Decimal::from(amount),
        kind: DamageKind::Attack,
        flags: DamageFlags {
            ignores_block: false,
            is_attack: true,
        },
        hit_count,
    })
}

fn block_self(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
) -> Vec<Effect> {
    ctx.state
        .player_creature_id()
        .map(|target| {
            vec![Effect::GainBlock {
                target,
                amount: Decimal::from(value(ctx, card, base, upgrade_delta)),
                source: Some(Source::Card(card)),
            }]
        })
        .unwrap_or_default()
}

fn lose_self_hp(ctx: &CardPlayCtx<'_>, card: CardInstanceId, amount: i32) -> Vec<Effect> {
    ctx.state
        .player_creature_id()
        .map(|target| {
            vec![Effect::LoseHp {
                target,
                amount: Decimal::from(amount),
                source: Some(Source::Card(card)),
            }]
        })
        .unwrap_or_default()
}

fn apply_self_power(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    power: crate::core::ids::PowerId,
    amount: i32,
) -> Vec<Effect> {
    ctx.state
        .player_creature_id()
        .map(|target| vec![apply_power(target, card, power, amount)])
        .unwrap_or_default()
}

fn apply_power(
    target: CreatureId,
    card: CardInstanceId,
    power: crate::core::ids::PowerId,
    amount: i32,
) -> Effect {
    Effect::ApplyPower {
        target,
        power,
        amount: Decimal::from(amount),
        source: Some(Source::Card(card)),
    }
}

fn value(ctx: &CardPlayCtx<'_>, card: CardInstanceId, base: i32, upgrade_delta: i32) -> i32 {
    base + if is_upgraded(ctx, card) {
        upgrade_delta
    } else {
        0
    }
}

fn is_upgraded(ctx: &CardPlayCtx<'_>, card: CardInstanceId) -> bool {
    ctx.state
        .card(card)
        .map(|card| card.upgraded)
        .unwrap_or(false)
}

fn hand_matching(ctx: &CardPlayCtx<'_>, filter: CardFilter) -> Vec<CardInstanceId> {
    let Some(combat) = ctx.state.combat() else {
        return Vec::new();
    };
    combat
        .player
        .piles
        .hand
        .iter()
        .copied()
        .filter(|card| card_matches_filter(ctx, *card, filter))
        .collect()
}

fn count_hand_type(ctx: &CardPlayCtx<'_>, card_type: CardType) -> i32 {
    hand_matching_type(ctx, card_type).len() as i32
}

fn hand_matching_type(ctx: &CardPlayCtx<'_>, card_type: CardType) -> Vec<CardInstanceId> {
    let Some(combat) = ctx.state.combat() else {
        return Vec::new();
    };
    combat
        .player
        .piles
        .hand
        .iter()
        .copied()
        .filter(|card| {
            ctx.state
                .card(*card)
                .and_then(|card| ctx.registry.cards.get(card.def))
                .map(|def| def.card_type == card_type)
                .unwrap_or(false)
        })
        .collect()
}

fn card_matches_filter(ctx: &CardPlayCtx<'_>, card: CardInstanceId, filter: CardFilter) -> bool {
    match filter {
        CardFilter::Any => true,
        CardFilter::Attack => ctx
            .state
            .card(card)
            .and_then(|card| ctx.registry.cards.get(card.def))
            .map(|def| def.card_type == CardType::Attack)
            .unwrap_or(false),
        CardFilter::NonAttack => ctx
            .state
            .card(card)
            .and_then(|card| ctx.registry.cards.get(card.def))
            .map(|def| def.card_type != CardType::Attack)
            .unwrap_or(false),
    }
}

fn count_all_tag(ctx: &CardPlayCtx<'_>, tag: CardTag) -> i32 {
    let Some(combat) = ctx.state.combat() else {
        return 0;
    };
    combat
        .cards
        .values()
        .filter(|card| {
            ctx.registry
                .cards
                .get(card.def)
                .map(|def| def.has_tag(tag))
                .unwrap_or(false)
        })
        .count() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ironclad_pool_matches_sts2_source_count() {
        assert_eq!(ironclad_card_defs().len(), 87);
    }

    #[test]
    fn standard_registry_contains_ironclad_cards_and_generated_tokens() {
        let registry = StaticRegistry::standard();
        for def in ironclad_card_defs() {
            assert!(registry.cards.contains(def.id), "missing {:?}", def.id);
        }
        assert!(registry.cards.contains(GIANT_ROCK));
    }

    #[test]
    fn upgraded_costs_are_defined_for_cost_upgrade_cards() {
        let registry = StaticRegistry::standard();
        let body_slam = registry.cards.get(BODY_SLAM).unwrap();
        assert_eq!(
            body_slam.costs_for(false).energy,
            crate::core::state::CardCost::Fixed(1)
        );
        assert_eq!(
            body_slam.costs_for(true).energy,
            crate::core::state::CardCost::Fixed(0)
        );

        let whirlwind = registry.cards.get(WHIRLWIND).unwrap();
        assert_eq!(
            whirlwind.costs_for(false).energy,
            crate::core::state::CardCost::X
        );
    }
}
