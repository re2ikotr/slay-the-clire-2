//! Hand-coded `Token` card pool.
//!
//! Tokens are short-lived cards generated during combat. They are small in
//! number and almost all behave like one-line wrappers around the helpers in
//! `super::helpers`, so we keep them in a single per-pool file rather than
//! splitting into one-file-per-card.
//!
//! `GIANT_ROCK` continues to live in `super::ironclad` because it predates this
//! migration and its def is consumed by other modules through
//! [`super::ironclad::giant_rock_def`]. We register it from here too so the
//! whole `Token` pool funnels through one entry point.

use super::helpers::{
    attack_effects, block_self, draw_cards, gain_energy, random_enemy_attack, value,
};
use super::{
    CardDef, CardKeyword, CardPlayCtx, CardPlayFn, CardPoolId, CardRarity, CardRules, CardTag,
    CardType, TargetType,
};
use crate::core::effect::Effect;
use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey};
use crate::core::query::{Decision, DecisionQuery, DecisionQueryKind, PreventReason};
use crate::core::rules::{prevent_by_current_listener, RuleCtx};
use crate::core::state::{CardCosts, PileKind};
use crate::registry::DefRegistry;

// Card IDs. Re-export so consumers can import them from `super::*` exactly as
// they do for ironclad. They're declared in `generated_cards.rs` because the
// catalog still references them; we just refer to those constants here.
use crate::content::generated_cards::{
    DISINTEGRATION, FUEL, LUMINESCE, MIND_ROT, MINION_DIVE_BOMB, MINION_SACRIFICE, MINION_STRIKE,
    SHIV, SLOTH, SOUL, SOVEREIGN_BLADE, SWEEPING_GAZE, WASTE_AWAY,
};

const KW_NONE: &[CardKeyword] = &[];
const KW_EXHAUST: &[CardKeyword] = &[CardKeyword::Exhaust];
const KW_EXHAUST_ETHEREAL: &[CardKeyword] = &[CardKeyword::Exhaust, CardKeyword::Ethereal];
const KW_EXHAUST_RETAIN: &[CardKeyword] = &[CardKeyword::Exhaust, CardKeyword::Retain];
const KW_RETAIN: &[CardKeyword] = &[CardKeyword::Retain];

const TAG_NONE: &[CardTag] = &[];
const TAG_SHIV: &[CardTag] = &[CardTag::Shiv];
const TAG_MINION: &[CardTag] = &[CardTag::Minion];
const TAG_STRIKE_MINION: &[CardTag] = &[CardTag::Strike, CardTag::Minion];
const TAG_OSTYATTACK: &[CardTag] = &[CardTag::OstyAttack];

#[derive(Clone, Copy)]
struct TokenSpec {
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

impl TokenSpec {
    fn def(self) -> CardDef {
        CardDef {
            id: self.id,
            loc_key: self.loc_key,
            pool: CardPoolId::Token,
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
            rules: card_rules_for(self.id),
        }
    }
}

/// Boilerplate for a token spec literal. The `LocKey` is constructed from the
/// ident so callsites stay readable.
macro_rules! token {
    (
        $id:ident, $type:ident, $rarity:ident, $target:ident,
        $cost:expr, $upcost:expr,
        $kw:expr, $upkw:expr, $tags:expr,
        $gen:expr, $play:ident
    ) => {
        TokenSpec {
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

const TOKEN_CARD_SPECS: &[TokenSpec] = &[
    // ---- Playable tokens ----
    token!(
        FUEL,
        Skill,
        Token,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        fuel_play
    ),
    token!(
        LUMINESCE,
        Skill,
        Token,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST_RETAIN,
        KW_NONE,
        TAG_NONE,
        true,
        luminesce_play
    ),
    token!(
        MINION_DIVE_BOMB,
        Attack,
        Token,
        Enemy,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_MINION,
        true,
        minion_dive_bomb_play
    ),
    token!(
        MINION_SACRIFICE,
        Skill,
        Token,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_MINION,
        true,
        minion_sacrifice_play
    ),
    token!(
        MINION_STRIKE,
        Attack,
        Token,
        Enemy,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_STRIKE_MINION,
        true,
        minion_strike_play
    ),
    token!(
        SHIV,
        Attack,
        Token,
        Enemy,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_SHIV,
        true,
        shiv_play
    ),
    token!(
        SOUL,
        Skill,
        Token,
        SelfTarget,
        CardCosts::energy(0),
        None,
        KW_EXHAUST,
        KW_NONE,
        TAG_NONE,
        true,
        soul_play
    ),
    token!(
        SOVEREIGN_BLADE,
        Attack,
        Token,
        Enemy,
        CardCosts::energy(2),
        Some(CardCosts::energy(1)),
        KW_RETAIN,
        KW_NONE,
        TAG_NONE,
        true,
        sovereign_blade_play
    ),
    token!(
        SWEEPING_GAZE,
        Attack,
        Token,
        RandomEnemy,
        CardCosts::energy(0),
        None,
        KW_EXHAUST_ETHEREAL,
        KW_NONE,
        TAG_OSTYATTACK,
        true,
        sweeping_gaze_play
    ),
    // ---- Status tokens ----
    //
    // Unplayable status cards added to the player's hand by enemy effects.
    // The runtime impact is implemented by powers/listeners outside this
    // file; we register them so card-pool lookups, draws, and discard piles
    // work.
    token!(
        DISINTEGRATION,
        Status,
        Status,
        None,
        UNPLAYABLE_COSTS,
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        false,
        no_play
    ),
    token!(
        MIND_ROT,
        Status,
        Status,
        None,
        UNPLAYABLE_COSTS,
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        false,
        no_play
    ),
    token!(
        SLOTH,
        Status,
        Status,
        None,
        UNPLAYABLE_COSTS,
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        false,
        no_play
    ),
    token!(
        WASTE_AWAY,
        Status,
        Status,
        None,
        UNPLAYABLE_COSTS,
        None,
        KW_NONE,
        KW_NONE,
        TAG_NONE,
        false,
        no_play
    ),
];

const UNPLAYABLE_COSTS: CardCosts = CardCosts {
    energy: crate::core::state::CardCost::Unplayable,
    stars: crate::core::state::CardCost::None,
};

pub fn register_token_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    for spec in TOKEN_CARD_SPECS {
        registry.register(spec.def());
    }
    // GIANT_ROCK is hand-coded inside `super::ironclad`; keep it part of the
    // Token pool for runtime lookup.
    registry.register(super::ironclad::giant_rock_def());
}

#[cfg(test)]
pub fn token_card_defs() -> Vec<CardDef> {
    TOKEN_CARD_SPECS.iter().map(|spec| spec.def()).collect()
}

// ---------------------------------------------------------------------------
// Play functions
// ---------------------------------------------------------------------------

fn no_play(_: &CardPlayCtx<'_>, _: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    Vec::new()
}

/// Fuel: gain 1 energy, draw 1 (2 upgraded). Energy never upgrades.
fn fuel_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    let mut effects = gain_energy(ctx, card, 1);
    let count = value(ctx, card, 1, 1) as u8;
    effects.extend(draw_cards(ctx, card, count));
    effects
}

/// Luminesce: gain 2 (3 upgraded) energy.
fn luminesce_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    gain_energy(ctx, card, value(ctx, card, 2, 1))
}

/// Minion Dive Bomb: 13 (16 upgraded) damage to a single enemy.
fn minion_dive_bomb_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    target
        .map(|t| attack_effects(ctx, card, t, value(ctx, card, 13, 3), 1))
        .unwrap_or_default()
}

/// Minion Sacrifice: gain 9 (12 upgraded) block.
fn minion_sacrifice_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    block_self(ctx, card, 9, 3)
}

/// Minion Strike: 6 (9 upgraded) damage, then draw 1.
fn minion_strike_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    let mut effects = target
        .map(|t| attack_effects(ctx, card, t, value(ctx, card, 6, 3), 1))
        .unwrap_or_default();
    effects.extend(draw_cards(ctx, card, 1));
    effects
}

/// Shiv: 4 (6 upgraded) damage to a single enemy.
fn shiv_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    target
        .map(|t| attack_effects(ctx, card, t, value(ctx, card, 4, 2), 1))
        .unwrap_or_default()
}

/// Soul: draw 2 (3 upgraded).
fn soul_play(ctx: &CardPlayCtx<'_>, card: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    draw_cards(ctx, card, value(ctx, card, 2, 1) as u8)
}

/// Sovereign Blade: 10 damage. Cost drops 2 → 1 when upgraded; damage stays.
fn sovereign_blade_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    target: Option<CreatureId>,
) -> Vec<Effect> {
    target
        .map(|t| attack_effects(ctx, card, t, 10, 1))
        .unwrap_or_default()
}

/// Sweeping Gaze: deal 10 (15 upgraded) damage to a random enemy.
///
/// In the C# source (sts2 `SweepingGaze.cs`) the attack is dealt by the
/// player's Osty pet and is skipped entirely if Osty is missing. The
/// migrated implementation here uses the player as the damage dealer and
/// always fires; wiring the Osty-dealer/Osty-missing branch is left as
/// follow-up work, since it requires plumbing not yet exposed by helpers.
///
/// The original catalog implementation also produced a *second* attack on
/// top of the Osty one (a quirk of the parametric `damage` + `osty_damage`
/// fields both being set). We drop that — the single attack matches the
/// canonical source.
fn sweeping_gaze_play(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    vec![random_enemy_attack(ctx, card, value(ctx, card, 10, 5), 1)]
}

// ---------------------------------------------------------------------------
// Rule dispatch
// ---------------------------------------------------------------------------

fn card_rules_for(id: CardId) -> CardRules {
    match id {
        SLOTH => CardRules {
            decide: Some(sloth_decide),
            ..CardRules::default()
        },
        _ => CardRules::default(),
    }
}

/// Sloth: prevents play of any card once the player has played 3 cards this
/// turn. Mirrors the original `catalog_card_decide` arm shared with NORMALITY.
fn sloth_decide(
    ctx: &RuleCtx<'_>,
    listener_card: CardInstanceId,
    query: &DecisionQuery,
) -> Decision {
    let in_hand = ctx
        .state
        .card(listener_card)
        .map(|c| c.pile.kind == PileKind::Hand)
        .unwrap_or(false);
    if !in_hand {
        return Decision::Allow;
    }
    let DecisionQueryKind::ShouldPlay { .. } = query.kind else {
        return Decision::Allow;
    };
    let played = ctx
        .state
        .combat()
        .map(|combat| combat.turn_stats.cards_played)
        .unwrap_or(0);
    if played >= 3 {
        prevent_by_current_listener(ctx, PreventReason::CannotPlay)
    } else {
        Decision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::GIANT_ROCK;
    use crate::registry::StaticRegistry;

    #[test]
    fn token_pool_size_matches_sts2_source_count() {
        // 9 playable + 4 status tokens authored here; GIANT_ROCK lives in
        // ironclad.rs and is registered separately.
        assert_eq!(token_card_defs().len(), 13);
    }

    #[test]
    fn standard_registry_contains_token_pool() {
        let registry = StaticRegistry::standard();
        for def in token_card_defs() {
            assert!(registry.cards.contains(def.id), "missing {:?}", def.id);
            assert_eq!(def.pool, CardPoolId::Token);
        }
        assert!(registry.cards.contains(GIANT_ROCK));
    }

    #[test]
    fn sloth_carries_decide_rule() {
        let registry = StaticRegistry::standard();
        let sloth = registry.cards.get(SLOTH).unwrap();
        assert!(
            sloth.rules.decide.is_some(),
            "SLOTH must keep its play-prevention rule"
        );
    }
}
