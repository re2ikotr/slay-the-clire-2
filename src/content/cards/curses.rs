//! Hand-coded `Curse` card pool.
//!
//! Curses are unplayable (or near-unplayable) cards added to the player's
//! deck. Most are pure shells — keywords + flavor text, no rules — but a
//! handful punish the player at end of turn or restrict play.
//!
//! Layout mirrors `super::tokens`:
//!   * one [`CurseSpec`] per card
//!   * per-card `*_play` functions when the card has `Fixed` cost (currently
//!     just `ENTHRALLED` and `SPORE_MIND`)
//!   * shared `card_rules_for(id)` dispatch building per-curse `CardRules`
//!     for `decide` / `on_event` hooks

use rust_decimal::Decimal;

use super::{
    CardDef, CardEventFn, CardKeyword, CardPlayCtx, CardPlayFn, CardPoolId, CardRarity, CardRules,
    CardTag, CardType, TargetType,
};
use crate::content::generated_cards::{
    ASCENDERS_BANE, BAD_LUCK, CLUMSY, CURSE_OF_THE_BELL, DEBT, DECAY, DOUBT, ENTHRALLED, FOLLY,
    GREED, GUILTY, INJURY, NORMALITY, POOR_SLEEP, REGRET, SHAME, SPORE_MIND, WRITHE,
};
use crate::content::powers::{FRAIL_POWER, WEAK};
use crate::core::effect::{DamageFlags, DamageKind, DamageOp, Effect, Source};
use crate::core::event::Event;
use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey};
use crate::core::query::{Decision, DecisionQuery, DecisionQueryKind, PreventReason};
use crate::core::rules::{prevent_by_current_listener, RuleCtx};
use crate::core::state::{CardCost, CardCosts, PileKind, Side};
use crate::registry::DefRegistry;

const KW_NONE: &[CardKeyword] = &[];
const KW_EXHAUST: &[CardKeyword] = &[CardKeyword::Exhaust];
const KW_ETERNAL: &[CardKeyword] = &[CardKeyword::Eternal];
const KW_UNPLAYABLE: &[CardKeyword] = &[CardKeyword::Unplayable];
const KW_UNPLAYABLE_ETERNAL: &[CardKeyword] = &[CardKeyword::Unplayable, CardKeyword::Eternal];
const KW_ETHEREAL_UNPLAYABLE: &[CardKeyword] = &[CardKeyword::Ethereal, CardKeyword::Unplayable];
const KW_ETHEREAL_UNPLAYABLE_ETERNAL: &[CardKeyword] = &[
    CardKeyword::Ethereal,
    CardKeyword::Unplayable,
    CardKeyword::Eternal,
];
const KW_INNATE_UNPLAYABLE: &[CardKeyword] = &[CardKeyword::Innate, CardKeyword::Unplayable];
const KW_INNATE_ETHEREAL_UNPLAYABLE_ETERNAL: &[CardKeyword] = &[
    CardKeyword::Innate,
    CardKeyword::Ethereal,
    CardKeyword::Unplayable,
    CardKeyword::Eternal,
];
const KW_RETAIN_UNPLAYABLE: &[CardKeyword] = &[CardKeyword::Retain, CardKeyword::Unplayable];

const TAG_NONE: &[CardTag] = &[];

const UNPLAYABLE_COSTS: CardCosts = CardCosts {
    energy: CardCost::Unplayable,
    stars: CardCost::None,
};

#[derive(Clone, Copy)]
struct CurseSpec {
    id: CardId,
    loc_key: LocKey,
    target: TargetType,
    base_costs: CardCosts,
    upgraded_costs: Option<CardCosts>,
    keywords: &'static [CardKeyword],
    upgraded_keywords: &'static [CardKeyword],
    play: CardPlayFn,
}

impl CurseSpec {
    fn def(self) -> CardDef {
        CardDef {
            id: self.id,
            loc_key: self.loc_key,
            pool: CardPoolId::Curse,
            card_type: CardType::Curse,
            rarity: CardRarity::Curse,
            target: self.target,
            base_costs: self.base_costs,
            upgraded_costs: self.upgraded_costs,
            keywords: self.keywords,
            upgraded_keywords: self.upgraded_keywords,
            tags: TAG_NONE,
            can_generate_in_combat: false,
            play: self.play,
            rules: card_rules_for(self.id),
        }
    }
}

macro_rules! curse {
    ($id:ident, $kw:expr, $play:ident) => {
        CurseSpec {
            id: $id,
            loc_key: LocKey::new(concat!("card.", stringify!($id))),
            target: TargetType::None,
            base_costs: UNPLAYABLE_COSTS,
            upgraded_costs: None,
            keywords: $kw,
            upgraded_keywords: KW_NONE,
            play: $play,
        }
    };
    // Variant for curses that have a non-unplayable base cost.
    (
        $id:ident, $kw:expr, $base_costs:expr, $play:ident
    ) => {
        CurseSpec {
            id: $id,
            loc_key: LocKey::new(concat!("card.", stringify!($id))),
            target: TargetType::None,
            base_costs: $base_costs,
            upgraded_costs: None,
            keywords: $kw,
            upgraded_keywords: KW_NONE,
            play: $play,
        }
    };
}

const CURSE_CARD_SPECS: &[CurseSpec] = &[
    // Pure unplayable shells (no rules, no body).
    curse!(ASCENDERS_BANE, KW_ETHEREAL_UNPLAYABLE_ETERNAL, no_play),
    curse!(CLUMSY, KW_ETHEREAL_UNPLAYABLE, no_play),
    curse!(CURSE_OF_THE_BELL, KW_UNPLAYABLE_ETERNAL, no_play),
    curse!(DEBT, KW_UNPLAYABLE, no_play),
    curse!(FOLLY, KW_INNATE_ETHEREAL_UNPLAYABLE_ETERNAL, no_play),
    curse!(GREED, KW_UNPLAYABLE_ETERNAL, no_play),
    curse!(GUILTY, KW_UNPLAYABLE, no_play),
    curse!(INJURY, KW_UNPLAYABLE, no_play),
    curse!(POOR_SLEEP, KW_RETAIN_UNPLAYABLE, no_play),
    curse!(WRITHE, KW_INNATE_UNPLAYABLE, no_play),

    // Turn-end-in-hand curses. Bodies live in `card_rules_for`'s `on_event`.
    curse!(BAD_LUCK, KW_UNPLAYABLE_ETERNAL, no_play),
    curse!(DECAY, KW_UNPLAYABLE, no_play),
    curse!(DOUBT, KW_UNPLAYABLE, no_play),
    curse!(REGRET, KW_UNPLAYABLE, no_play),
    curse!(SHAME, KW_UNPLAYABLE, no_play),

    // Play-prevention curses (rules are in `card_rules_for`'s `decide`).
    curse!(NORMALITY, KW_UNPLAYABLE, no_play),

    // Curses with actual playable cost. Their bodies are real play functions.
    curse!(ENTHRALLED, KW_ETERNAL, CardCosts::energy(2), enthralled_play),
    curse!(SPORE_MIND, KW_EXHAUST, CardCosts::energy(1), spore_mind_play),
];

pub fn register_curse_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    for spec in CURSE_CARD_SPECS {
        registry.register(spec.def());
    }
}

#[cfg(test)]
pub fn curse_card_defs() -> Vec<CardDef> {
    CURSE_CARD_SPECS.iter().map(|spec| spec.def()).collect()
}

// ---------------------------------------------------------------------------
// Play functions
// ---------------------------------------------------------------------------

fn no_play(_: &CardPlayCtx<'_>, _: CardInstanceId, _: Option<CreatureId>) -> Vec<Effect> {
    Vec::new()
}

/// Enthralled: must be played before other cards while in hand, but playing
/// it has no body — the curse is paying its energy cost as the punishment.
fn enthralled_play(
    _ctx: &CardPlayCtx<'_>,
    _card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}

/// Spore Mind: 1-cost Exhaust skill with no further effect. Playing it just
/// burns 1 energy and exhausts the card; the engine handles both via the
/// keyword and cost system.
fn spore_mind_play(
    _ctx: &CardPlayCtx<'_>,
    _card: CardInstanceId,
    _: Option<CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// Rule dispatch
// ---------------------------------------------------------------------------

fn card_rules_for(id: CardId) -> CardRules {
    let on_event = match id {
        BAD_LUCK => Some(bad_luck_on_event as CardEventFn),
        DECAY => Some(decay_on_event as CardEventFn),
        DOUBT => Some(doubt_on_event as CardEventFn),
        REGRET => Some(regret_on_event as CardEventFn),
        SHAME => Some(shame_on_event as CardEventFn),
        _ => None,
    };
    let decide = match id {
        NORMALITY => Some(normality_decide as _),
        ENTHRALLED => Some(enthralled_decide as _),
        _ => None,
    };
    CardRules {
        on_event,
        decide,
        ..CardRules::default()
    }
}

// ---- on_event handlers ----

/// Helper: only fire if the curse is in the player's hand and the event is the
/// player's own end of turn.
fn end_of_player_turn_in_hand(
    ctx: &RuleCtx<'_>,
    card: CardInstanceId,
    event: &Event,
) -> bool {
    if !matches!(event, Event::TurnEnded { side: Side::Player }) {
        return false;
    }
    ctx.state
        .card(card)
        .map(|c| c.pile.kind == PileKind::Hand)
        .unwrap_or(false)
}

fn bad_luck_on_event(ctx: &RuleCtx<'_>, card: CardInstanceId, event: &Event) -> Vec<Effect> {
    if !end_of_player_turn_in_hand(ctx, card, event) {
        return Vec::new();
    }
    let Some(target) = ctx.state.player_creature_id() else {
        return Vec::new();
    };
    vec![Effect::LoseHp {
        target,
        amount: Decimal::from(13),
        source: Some(Source::Card(card)),
    }]
}

fn decay_on_event(ctx: &RuleCtx<'_>, card: CardInstanceId, event: &Event) -> Vec<Effect> {
    if !end_of_player_turn_in_hand(ctx, card, event) {
        return Vec::new();
    }
    let Some(target) = ctx.state.player_creature_id() else {
        return Vec::new();
    };
    // The catalog implementation treated DECAY damage as `LifeLoss` with
    // `ignores_block: true`. Preserve that exactly.
    vec![Effect::DealDamage(DamageOp {
        source: Some(Source::Card(card)),
        dealer: None,
        target,
        base_amount: Decimal::from(2),
        kind: DamageKind::LifeLoss,
        flags: DamageFlags {
            ignores_block: true,
        },
    })]
}

fn doubt_on_event(ctx: &RuleCtx<'_>, card: CardInstanceId, event: &Event) -> Vec<Effect> {
    if !end_of_player_turn_in_hand(ctx, card, event) {
        return Vec::new();
    }
    let Some(target) = ctx.state.player_creature_id() else {
        return Vec::new();
    };
    vec![Effect::ApplyPower {
        target,
        power: WEAK,
        amount: Decimal::from(1),
        source: Some(Source::Card(card)),
    }]
}

fn shame_on_event(ctx: &RuleCtx<'_>, card: CardInstanceId, event: &Event) -> Vec<Effect> {
    if !end_of_player_turn_in_hand(ctx, card, event) {
        return Vec::new();
    }
    let Some(target) = ctx.state.player_creature_id() else {
        return Vec::new();
    };
    vec![Effect::ApplyPower {
        target,
        power: FRAIL_POWER,
        amount: Decimal::from(1),
        source: Some(Source::Card(card)),
    }]
}

fn regret_on_event(ctx: &RuleCtx<'_>, card: CardInstanceId, event: &Event) -> Vec<Effect> {
    if !end_of_player_turn_in_hand(ctx, card, event) {
        return Vec::new();
    }
    let Some(target) = ctx.state.player_creature_id() else {
        return Vec::new();
    };
    let hand_size = ctx
        .state
        .combat()
        .map(|combat| combat.player.piles.hand.len() as i32)
        .unwrap_or(0)
        .max(0);
    vec![Effect::LoseHp {
        target,
        amount: Decimal::from(hand_size),
        source: Some(Source::Card(card)),
    }]
}

// ---- decide handlers ----

/// Normality: prevents play when the player has already played 3 or more cards
/// this turn.
fn normality_decide(
    ctx: &RuleCtx<'_>,
    listener_card: CardInstanceId,
    query: &DecisionQuery,
) -> Decision {
    if !is_in_hand(ctx, listener_card) {
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

/// Enthralled: while in hand it must be played before any other card.
fn enthralled_decide(
    ctx: &RuleCtx<'_>,
    listener_card: CardInstanceId,
    query: &DecisionQuery,
) -> Decision {
    if !is_in_hand(ctx, listener_card) {
        return Decision::Allow;
    }
    let DecisionQueryKind::ShouldPlay { card, .. } = query.kind else {
        return Decision::Allow;
    };
    if card == listener_card {
        Decision::Allow
    } else {
        prevent_by_current_listener(ctx, PreventReason::Custom("enthralled"))
    }
}

fn is_in_hand(ctx: &RuleCtx<'_>, card: CardInstanceId) -> bool {
    ctx.state
        .card(card)
        .map(|c| c.pile.kind == PileKind::Hand)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::StaticRegistry;

    #[test]
    fn curse_pool_size_matches_sts2_source_count() {
        assert_eq!(curse_card_defs().len(), 18);
    }

    #[test]
    fn standard_registry_contains_curse_pool() {
        let registry = StaticRegistry::standard();
        for def in curse_card_defs() {
            assert!(registry.cards.contains(def.id), "missing {:?}", def.id);
            assert_eq!(def.pool, CardPoolId::Curse);
        }
    }

    #[test]
    fn turn_end_curses_carry_on_event_rule() {
        let registry = StaticRegistry::standard();
        for id in [BAD_LUCK, DECAY, DOUBT, REGRET, SHAME] {
            let def = registry.cards.get(id).expect("curse registered");
            assert!(
                def.rules.on_event.is_some(),
                "{:?} must keep its turn-end-in-hand rule",
                id
            );
        }
    }

    #[test]
    fn play_preventing_curses_carry_decide_rule() {
        let registry = StaticRegistry::standard();
        for id in [NORMALITY, ENTHRALLED] {
            let def = registry.cards.get(id).expect("curse registered");
            assert!(
                def.rules.decide.is_some(),
                "{:?} must keep its play-prevention rule",
                id
            );
        }
    }
}
