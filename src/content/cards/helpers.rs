//! Shared card-play helpers.
//!
//! These were originally private to `ironclad.rs`. They are extracted here so
//! other pool modules (`silent`, `defect`, …) can build their own per-card
//! play functions without re-implementing the same primitives.
//!
//! Visibility is `pub(crate)` so any module inside the crate can use them, but
//! they are not part of the public API.

use rust_decimal::Decimal;

use super::{CardPlayCtx, CardTag, CardType};
use crate::core::effect::{
    CardFilter, DamageAllEnemiesOp, DamageFlags, DamageKind, DamageOp, Effect, RandomDamageOp,
    Source,
};
use crate::core::ids::{CardInstanceId, CreatureId, PowerId};
use crate::core::state::ResourceKind;

/// Strike-pattern helper: target-required attack with a base damage and an
/// optional upgrade delta, applied `hits` times.
pub(crate) fn strike_like(
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

/// Build `hits` independent `DealDamage` effects against `target` for `amount`
/// damage each. Caller is expected to have already applied any
/// upgrade-dependent value resolution.
pub(crate) fn attack_effects(
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
                },
            })
        })
        .collect()
}

/// AoE attack against every alive enemy, repeated `hit_count` times.
pub(crate) fn all_enemy_attack(
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
        },
        hit_count,
    })
}

/// Single-effect random-enemy attack repeated `hit_count` times.
pub(crate) fn random_enemy_attack(
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
        },
        hit_count,
    })
}

/// Gain block on the player creature. Returns empty if there is no current
/// player creature (e.g. outside of combat).
pub(crate) fn block_self(
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

/// Player loses HP from the source card.
pub(crate) fn lose_self_hp(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    amount: i32,
) -> Vec<Effect> {
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

/// Player draws `count` cards.
pub(crate) fn draw_cards(ctx: &CardPlayCtx<'_>, card: CardInstanceId, count: u8) -> Vec<Effect> {
    ctx.state
        .card(card)
        .map(|card_state| {
            vec![Effect::DrawCards {
                player: card_state.owner,
                count,
            }]
        })
        .unwrap_or_default()
}

/// Player gains `amount` of the given resource (Energy or Stars).
pub(crate) fn gain_resource(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    resource: ResourceKind,
    amount: i32,
) -> Vec<Effect> {
    ctx.state
        .card(card)
        .map(|card_state| {
            vec![Effect::GainResource {
                player: card_state.owner,
                resource,
                amount,
            }]
        })
        .unwrap_or_default()
}

/// Player gains `amount` energy.
pub(crate) fn gain_energy(ctx: &CardPlayCtx<'_>, card: CardInstanceId, amount: i32) -> Vec<Effect> {
    gain_resource(ctx, card, ResourceKind::Energy, amount)
}

/// Apply a power to the player creature.
pub(crate) fn apply_self_power(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    power: PowerId,
    amount: i32,
) -> Vec<Effect> {
    ctx.state
        .player_creature_id()
        .map(|target| vec![apply_power(target, card, power, amount)])
        .unwrap_or_default()
}

/// Apply a power to an arbitrary creature. Returns the bare `Effect` (not a
/// `Vec`) so callers can mix it inline with other effects.
pub(crate) fn apply_power(
    target: CreatureId,
    card: CardInstanceId,
    power: PowerId,
    amount: i32,
) -> Effect {
    Effect::ApplyPower {
        target,
        power,
        amount: Decimal::from(amount),
        source: Some(Source::Card(card)),
    }
}

/// Resolve a `(base, upgrade_delta)` value against the card's upgrade state.
pub(crate) fn value(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
) -> i32 {
    base + if is_upgraded(ctx, card) {
        upgrade_delta
    } else {
        0
    }
}

/// Whether the given card instance is upgraded.
pub(crate) fn is_upgraded(ctx: &CardPlayCtx<'_>, card: CardInstanceId) -> bool {
    ctx.state
        .card(card)
        .map(|card| card.upgraded)
        .unwrap_or(false)
}

/// All cards in the player's hand matching `filter`.
pub(crate) fn hand_matching(
    ctx: &CardPlayCtx<'_>,
    filter: CardFilter,
) -> Vec<CardInstanceId> {
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

/// Number of cards in the player's hand of the given type.
pub(crate) fn count_hand_type(ctx: &CardPlayCtx<'_>, card_type: CardType) -> i32 {
    hand_matching_type(ctx, card_type).len() as i32
}

/// All cards in the player's hand of the given type.
pub(crate) fn hand_matching_type(
    ctx: &CardPlayCtx<'_>,
    card_type: CardType,
) -> Vec<CardInstanceId> {
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

/// Whether a single card matches a `CardFilter`. Used to back hand/pile
/// filtering. Mirrors the engine's filter semantics so play helpers and
/// engine-side queries agree.
pub(crate) fn card_matches_filter(
    ctx: &CardPlayCtx<'_>,
    card: CardInstanceId,
    filter: CardFilter,
) -> bool {
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

/// Count of all card instances (across all piles in current combat) carrying
/// the given tag.
pub(crate) fn count_all_tag(ctx: &CardPlayCtx<'_>, tag: CardTag) -> i32 {
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
