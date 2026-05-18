use rust_decimal::Decimal;

use crate::content::cards::*;
use crate::content::powers::*;
use crate::core::effect::{DamageKind, Source};
use crate::core::ids::{CardInstanceId, CreatureId, PowerId};
use crate::core::query::{BlockCalc, DamageCalc, ResourceCostCalc};
use crate::core::rules::RulePipeline;
use crate::core::state::{
    decimal_to_i32_trunc, CardCost, CardCosts, CardCounter, GameState, ResourceKind,
};
use crate::registry::StaticRegistry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTextScope {
    Hand,
    Pile,
}

#[derive(Clone, Copy)]
pub struct CardTextCtx<'a> {
    pub state: &'a GameState,
    pub registry: &'a StaticRegistry,
    pub target: Option<CreatureId>,
    pub scope: CardTextScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardText {
    pub lines: Vec<CardTextLine>,
    pub keywords: Vec<CardKeyword>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardTextLine {
    pub eng: String,
    pub zhs: String,
}

pub fn describe_card(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> CardText {
    let lines = describe_lines(ctx, card);
    let keywords = describe_keywords(ctx, card);
    CardText { lines, keywords }
}

pub fn display_costs(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> CardCosts {
    let Some(card_state) = ctx.state.card(card) else {
        return CardCosts::default();
    };
    let def_costs = ctx
        .registry
        .cards
        .get(card_state.def)
        .map(|def| def.costs_for(card_state.upgraded))
        .unwrap_or(card_state.costs);
    let mut costs = card_state.costs_with_temporary(def_costs);
    if ctx.scope != CardTextScope::Hand {
        return costs;
    }
    let Some(player) = ctx.state.player_id() else {
        return costs;
    };
    costs.energy = display_resource_cost(ctx, player, card, ResourceKind::Energy, costs.energy);
    costs.stars = display_resource_cost(ctx, player, card, ResourceKind::Stars, costs.stars);
    costs
}

fn display_resource_cost(
    ctx: &CardTextCtx<'_>,
    player: crate::core::ids::PlayerId,
    card: CardInstanceId,
    resource: ResourceKind,
    cost: CardCost,
) -> CardCost {
    let CardCost::Fixed(base_cost) = cost else {
        return cost;
    };
    let calc = ResourceCostCalc {
        player,
        card,
        resource,
        base_cost,
        cost: base_cost,
    };
    let (calc, _) = RulePipeline::modify_resource_cost(ctx.registry, ctx.state, calc);
    CardCost::Fixed(calc.cost.max(0))
}

fn describe_keywords(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardKeyword> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    let Some(def) = ctx.registry.cards.get(card_state.def) else {
        return Vec::new();
    };
    let mut keywords = Vec::new();
    for keyword in def.keywords {
        push_keyword(&mut keywords, *keyword);
    }
    if card_state.upgraded {
        for keyword in def.upgraded_keywords {
            push_keyword(&mut keywords, *keyword);
        }
    }
    if card_state.flags.ethereal {
        push_keyword(&mut keywords, CardKeyword::Ethereal);
    }
    if card_state.flags.temporary {
        push_keyword(&mut keywords, CardKeyword::Temporary);
    }
    if card_state.flags.purge_on_use {
        push_keyword(&mut keywords, CardKeyword::PurgeOnUse);
    }
    if card_state.flags.zero_cost_this_turn {
        push_keyword(&mut keywords, CardKeyword::FreeThisTurn);
    }
    if ctx.scope == CardTextScope::Hand
        && def.card_type == CardType::Skill
        && ctx
            .state
            .player_creature_id()
            .map(|creature| ctx.state.has_power(creature, CORRUPTION_POWER))
            .unwrap_or(false)
    {
        push_keyword(&mut keywords, CardKeyword::Exhaust);
    }
    keywords
}

fn push_keyword(keywords: &mut Vec<CardKeyword>, keyword: CardKeyword) {
    if !keywords.contains(&keyword) {
        keywords.push(keyword);
    }
}

fn describe_lines(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardTextLine> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    match card_state.def {
        AGGRESSION => vec![l(
            "At the start of your turn, put a random Attack from your discard pile into your hand and upgrade it.",
            "在你的回合开始时，将弃牌堆中的1张随机攻击牌置入手牌并升级。",
        )],
        ANGER => vec![
            damage(ctx, card, 6, 2),
            l("Add a copy of this card into your discard pile.", "将此牌的一张复制加入弃牌堆。"),
        ],
        ARMAMENTS => vec![
            block(ctx, card, 5, 0),
            if upgraded(ctx, card) {
                l("Upgrade ALL cards in your hand.", "升级你手牌中的所有牌。")
            } else {
                l("Upgrade a card in your hand.", "升级你手牌中的1张牌。")
            },
        ],
        ASHEN_STRIKE => vec![
            damage_exact(
                preview_damage(ctx, card, 6 + exhaust_count(ctx) * if upgraded(ctx, card) { 4 } else { 3 }),
            ),
            l(
                format!(
                    "Deals {} additional damage for each card in your exhaust pile.",
                    if upgraded(ctx, card) { 4 } else { 3 }
                ),
                format!(
                    "你的消耗牌堆中每有1张牌，额外造成{}点伤害。",
                    if upgraded(ctx, card) { 4 } else { 3 }
                ),
            ),
        ],
        BARRICADE => vec![l(
            "Block is not removed at the start of your turn.",
            "你的回合开始时不再失去格挡。",
        )],
        BASH => vec![
            damage(ctx, card, 8, 2),
            apply_power("Vulnerable", "易伤", val(ctx, card, 2, 1)),
        ],
        BATTLE_TRANCE => vec![
            draw(val(ctx, card, 3, 1)),
            l("You cannot draw additional cards this turn.", "本回合你不能再抽牌。"),
        ],
        BLOOD_WALL => vec![lose_hp(2), block(ctx, card, 16, 4)],
        BLOODLETTING => vec![lose_hp(3), gain_energy(val(ctx, card, 2, 1))],
        BLUDGEON => vec![damage(ctx, card, 32, 10)],
        BODY_SLAM => vec![l(
            format!(
                "Deal damage equal to your Block. (Deals {} damage.)",
                preview_damage(ctx, card, player_block(ctx))
            ),
            format!(
                "造成等同于你格挡的伤害。（造成{}点伤害。）",
                preview_damage(ctx, card, player_block(ctx))
            ),
        )],
        BRAND => vec![
            lose_hp(1),
            l("Exhaust 1 random card in your hand.", "随机消耗你手牌中的1张牌。"),
            gain_power("Strength", "力量", val(ctx, card, 1, 1)),
        ],
        BREAK => vec![
            damage(ctx, card, 20, 10),
            apply_power("Vulnerable", "易伤", if upgraded(ctx, card) { 7 } else { 5 }),
        ],
        BREAKTHROUGH => vec![lose_hp(1), damage_all(ctx, card, 9, 4)],
        BULLY => {
            let per = if upgraded(ctx, card) { 3 } else { 2 };
            let vuln = ctx
                .target
                .map(|target| ctx.state.power_amount(target, VULNERABLE))
                .unwrap_or(0);
            vec![
                damage_exact(preview_damage(ctx, card, 4 + per * vuln)),
                l(
                    format!("Deals {per} additional damage for each Vulnerable on the enemy."),
                    format!("敌人每有1层易伤，额外造成{per}点伤害。"),
                ),
            ]
        }
        BURNING_PACT => vec![
            l("Exhaust 1 random card in your hand.", "随机消耗你手牌中的1张牌。"),
            draw(val(ctx, card, 2, 1)),
        ],
        CASCADE => vec![if upgraded(ctx, card) {
            l("Play the top X+1 cards of your draw pile.", "打出你抽牌堆顶的X+1张牌。")
        } else {
            l("Play the top X cards of your draw pile.", "打出你抽牌堆顶的X张牌。")
        }],
        CINDER => vec![
            damage(ctx, card, 18, 6),
            l("Exhaust the top card of your draw pile.", "消耗你抽牌堆顶的1张牌。"),
        ],
        COLOSSUS => vec![
            block(ctx, card, 5, 3),
            l("You receive 50% less damage from Vulnerable enemies this turn.", "本回合中，你从易伤敌人处受到的伤害减少50%。"),
        ],
        CONFLAGRATION => {
            let previous = previous_attacks_played(ctx);
            let per = if upgraded(ctx, card) { 3 } else { 2 };
            vec![
                damage_all_exact(preview_damage(ctx, card, 8 + previous * per)),
                l(
                    format!("Deals {per} additional damage for each other Attack you've played this turn."),
                    format!("本回合你每打出过1张其他攻击牌，额外造成{per}点伤害。"),
                ),
            ]
        }
        CORRUPTION => vec![l(
            "Skills cost 0 Energy. Whenever you play a Skill, Exhaust it.",
            "技能牌耗能变为0。每当你打出一张技能牌，将其消耗。",
        )],
        CRIMSON_MANTLE => vec![l(
            format!(
                "At the start of your turn, lose 1 HP and gain {} Block.",
                val(ctx, card, 8, 2)
            ),
            format!(
                "在你的回合开始时，失去1点生命并获得{}点格挡。",
                val(ctx, card, 8, 2)
            ),
        )],
        CRUELTY => vec![l(
            format!(
                "Vulnerable enemies take an additional {}% damage.",
                if upgraded(ctx, card) { 50 } else { 25 }
            ),
            format!(
                "易伤敌人受到的伤害额外增加{}%。",
                if upgraded(ctx, card) { 50 } else { 25 }
            ),
        )],
        DARK_EMBRACE => vec![l(
            "Whenever a card is Exhausted, draw 1 card.",
            "每当有牌被消耗时，抽1张牌。",
        )],
        DEFEND_IRONCLAD => vec![block(ctx, card, 5, 3)],
        DEMON_FORM => vec![l(
            format!("At the start of your turn, gain {} Strength.", val(ctx, card, 2, 1)),
            format!("在你的回合开始时，获得{}点力量。", val(ctx, card, 2, 1)),
        )],
        DEMONIC_SHIELD => vec![
            lose_hp(1),
            l(
                format!("An ally gains Block equal to your current Block. ({} Block.)", player_block(ctx)),
                format!("一名友方获得等同于你当前格挡的格挡。（{}点格挡。）", player_block(ctx)),
            ),
        ],
        DISMANTLE => vec![
            damage_times(ctx, card, 8, 2, if target_has(ctx, VULNERABLE) { 2 } else { 1 }),
            l("Hits twice if the enemy is Vulnerable.", "如果敌人拥有易伤，攻击2次。"),
        ],
        DOMINATE => {
            let applied = if upgraded(ctx, card) { 2 } else { 1 };
            let after = ctx
                .target
                .map(|target| ctx.state.power_amount(target, VULNERABLE) + applied)
                .unwrap_or(applied);
            vec![
                apply_power("Vulnerable", "易伤", applied),
                gain_power("Strength", "力量", after),
            ]
        }
        DRUM_OF_BATTLE => vec![
            draw(val(ctx, card, 2, 1)),
            l("Whenever you play an Attack, draw 1 card.", "每当你打出一张攻击牌，抽1张牌。"),
        ],
        EVIL_EYE => {
            let repeats = if cards_exhausted_this_turn(ctx) > 0 { 2 } else { 1 };
            vec![block_times(ctx, card, 8, 3, repeats)]
        }
        EXPECT_A_FIGHT => vec![
            l(
                format!("Gain Energy equal to the number of Attacks in your hand. ({} Energy.)", count_hand_type(ctx, CardType::Attack)),
                format!("获得等同于你手牌中攻击牌数量的能量。（{}点能量。）", count_hand_type(ctx, CardType::Attack)),
            ),
            l("You cannot gain additional Energy this turn.", "本回合你不能再获得能量。"),
        ],
        FEED => vec![
            damage(ctx, card, 10, 2),
            l(
                format!("If this kills an enemy, gain {} Max HP.", val(ctx, card, 3, 1)),
                format!("如果此牌杀死敌人，获得{}点最大生命。", val(ctx, card, 3, 1)),
            ),
        ],
        FEEL_NO_PAIN => vec![l(
            format!("Whenever a card is Exhausted, gain {} Block.", val(ctx, card, 3, 1)),
            format!("每当有牌被消耗时，获得{}点格挡。", val(ctx, card, 3, 1)),
        )],
        FIEND_FIRE => vec![
            l("Exhaust your hand.", "消耗你的所有手牌。"),
            damage_times(ctx, card, 7, 3, hand_len(ctx) as u8),
        ],
        FIGHT_ME => vec![
            damage_times(ctx, card, 5, 1, 2),
            gain_power("Strength", "力量", if upgraded(ctx, card) { 3 } else { 2 }),
            l("The enemy gains 1 Strength.", "敌人获得1点力量。"),
        ],
        FLAME_BARRIER => vec![
            block(ctx, card, 12, 4),
            l(
                format!("Whenever you are attacked this turn, deal {} damage back.", val(ctx, card, 4, 2)),
                format!("本回合每当你受到攻击时，反击造成{}点伤害。", val(ctx, card, 4, 2)),
            ),
        ],
        FORGOTTEN_RITUAL => vec![l(
            format!("If a card was Exhausted this turn, gain {} Energy.", val(ctx, card, 3, 1)),
            format!("如果本回合有牌被消耗，获得{}点能量。", val(ctx, card, 3, 1)),
        )],
        HAVOC => vec![l(
            "Play the top card of your draw pile and Exhaust it.",
            "打出你抽牌堆顶的1张牌，并将其消耗。",
        )],
        HEADBUTT => vec![
            damage(ctx, card, 9, 3),
            l("Put the top card of your discard pile on top of your draw pile.", "将你弃牌堆顶的1张牌置于抽牌堆顶。"),
        ],
        HELLRAISER => vec![l(
            "At the start of your turn, add a random Attack into your hand.",
            "在你的回合开始时，将1张随机攻击牌加入手牌。",
        )],
        HEMOKINESIS => vec![lose_hp(2), damage(ctx, card, 14, 5)],
        HOWL_FROM_BEYOND => vec![damage_all(ctx, card, 16, 5)],
        IMPERVIOUS => vec![block(ctx, card, 30, 10)],
        INFERNAL_BLADE => vec![l(
            "Add a random Attack into your hand. It costs 0 this turn.",
            "将1张随机攻击牌加入手牌。它在本回合耗能为0。",
        )],
        INFERNO => vec![l(
            format!("At the end of your turn, deal {} damage to ALL enemies.", val(ctx, card, 6, 3)),
            format!("在你的回合结束时，对所有敌人造成{}点伤害。", val(ctx, card, 6, 3)),
        )],
        INFLAME => vec![gain_power("Strength", "力量", val(ctx, card, 2, 1))],
        IRON_WAVE => vec![block(ctx, card, 5, 2), damage(ctx, card, 5, 2)],
        JUGGERNAUT => vec![l(
            format!("Whenever you gain Block, deal {} damage to a random enemy.", val(ctx, card, 5, 2)),
            format!("每当你获得格挡时，对随机敌人造成{}点伤害。", val(ctx, card, 5, 2)),
        )],
        JUGGLING => vec![l(
            "At the start of your turn, draw 1 card.",
            "在你的回合开始时，抽1张牌。",
        )],
        MANGLE => vec![
            damage(ctx, card, 15, 5),
            l(
                format!("Enemy loses {} Strength this turn.", val(ctx, card, 10, 5)),
                format!("敌人本回合失去{}点力量。", val(ctx, card, 10, 5)),
            ),
        ],
        MOLTEN_FIST => vec![
            damage(ctx, card, 10, 4),
            l("Double the enemy's Vulnerable.", "使敌人的易伤翻倍。"),
        ],
        NOT_YET => vec![heal(val(ctx, card, 10, 3))],
        OFFERING => vec![lose_hp(6), gain_energy(2), draw(val(ctx, card, 3, 2))],
        ONE_TWO_PUNCH => vec![l(
            format!(
                "This turn, your next {} {} played an extra time.",
                val(ctx, card, 1, 1),
                plural(val(ctx, card, 1, 1), "Attack is", "Attacks are")
            ),
            format!("本回合，你接下来{}张攻击牌额外打出1次。", val(ctx, card, 1, 1)),
        )],
        PACTS_END => vec![
            l("Can only be played if you have 3 or more cards in your exhaust pile.", "只能在你的消耗牌堆中有3张或更多牌时打出。"),
            damage_all(ctx, card, 17, 6),
        ],
        PERFECTED_STRIKE => {
            let strikes = count_all_tag(ctx, CardTag::Strike);
            let per = if upgraded(ctx, card) { 3 } else { 2 };
            vec![
                damage_exact(preview_damage(ctx, card, 6 + strikes * per)),
                l(
                    format!("Deals {per} additional damage for ALL your cards containing \"Strike\"."),
                    format!("你的所有含“打击”的牌每有1张，额外造成{per}点伤害。"),
                ),
            ]
        }
        PILLAGE => vec![
            damage(ctx, card, 6, 3),
            l("Draw cards until you draw a non-Attack card.", "抽牌，直到抽到1张非攻击牌。"),
        ],
        POMMEL_STRIKE => vec![damage(ctx, card, 9, 1), draw(val(ctx, card, 1, 1))],
        PRIMAL_FORCE => vec![l(
            if upgraded(ctx, card) {
                "Transform all Attacks in your hand into Giant Rock+."
            } else {
                "Transform all Attacks in your hand into Giant Rock."
            },
            if upgraded(ctx, card) {
                "将你手牌中的所有攻击牌变为巨石+。"
            } else {
                "将你手牌中的所有攻击牌变为巨石。"
            },
        )],
        PYRE => vec![l("Gain 1 Energy at the start of each turn.", "每回合开始时获得1点能量。")],
        RAGE => vec![l(
            format!("Whenever you play an Attack this turn, gain {} Block.", val(ctx, card, 3, 2)),
            format!("本回合每当你打出一张攻击牌，获得{}点格挡。", val(ctx, card, 3, 2)),
        )],
        RAMPAGE => {
            let bonus = ctx
                .state
                .card(card)
                .map(|card| card.counter(CardCounter::DamageIncrease))
                .unwrap_or(0);
            vec![
                damage_exact(preview_damage(ctx, card, 9 + bonus)),
                l(
                    format!("Increase this card's damage by {} this combat.", if upgraded(ctx, card) { 9 } else { 5 }),
                    format!("本场战斗中，此牌伤害提高{}点。", if upgraded(ctx, card) { 9 } else { 5 }),
                ),
            ]
        }
        RUPTURE => vec![l(
            format!("Whenever you lose HP on your turn, gain {} Strength.", val(ctx, card, 1, 1)),
            format!("每当你在自己的回合失去生命，获得{}点力量。", val(ctx, card, 1, 1)),
        )],
        SECOND_WIND => vec![
            l("Exhaust all non-Attack cards in your hand.", "消耗你手牌中的所有非攻击牌。"),
            l(
                format!("Gain {} Block for each card Exhausted.", block_amount(ctx, card, 5, 2)),
                format!("每消耗1张牌，获得{}点格挡。", block_amount(ctx, card, 5, 2)),
            ),
        ],
        SETUP_STRIKE => vec![
            damage(ctx, card, 7, 2),
            gain_power("Strength", "力量", 2),
            l("At end of turn, lose 2 Strength.", "回合结束时失去2点力量。"),
        ],
        SHRUG_IT_OFF => vec![block(ctx, card, 8, 3), draw(1)],
        SPITE => {
            let hits = if hp_lost_by_player_this_turn(ctx) > 0 {
                val(ctx, card, 2, 1) as u8
            } else {
                1
            };
            vec![
                damage_times(ctx, card, 5, 0, hits),
                l("Hits more times if you lost HP this turn.", "如果你本回合失去过生命，攻击更多次。"),
            ]
        }
        STAMPEDE => vec![l(
            "Whenever you play an Attack, gain 1 Block for each Attack played this turn.",
            "每当你打出一张攻击牌，根据本回合已打出的攻击牌数量获得格挡。",
        )],
        STOKE => vec![l(
            format!("Exhaust your hand, then draw that many cards. ({} cards.)", hand_len(ctx)),
            format!("消耗你的手牌，然后抽等量的牌。（{}张。）", hand_len(ctx)),
        )],
        STOMP => vec![damage_all(ctx, card, 12, 3)],
        STONE_ARMOR => vec![gain_power("Plating", "护甲", val(ctx, card, 4, 2))],
        STRIKE_IRONCLAD => vec![damage(ctx, card, 6, 3)],
        SWORD_BOOMERANG => vec![damage_random_times(ctx, card, 3, val(ctx, card, 3, 1) as u8)],
        TANK => vec![l("The first time you gain Block each turn, double it.", "每回合你第一次获得格挡时，使其翻倍。")],
        TAUNT => vec![
            block(ctx, card, 7, 1),
            apply_power("Vulnerable", "易伤", if upgraded(ctx, card) { 2 } else { 1 }),
        ],
        TEAR_ASUNDER => {
            let hits = 1 + hp_loss_events_by_player(ctx) as u8;
            vec![
                damage_times(ctx, card, 5, 2, hits),
                l("Hits an additional time for each time you have lost HP this combat.", "本场战斗中你每次失去生命，额外攻击1次。"),
            ]
        }
        THRASH => vec![
            damage_times(ctx, card, 4, 2, 2),
            l("Exhaust a random Attack in your hand.", "随机消耗你手牌中的1张攻击牌。"),
        ],
        THUNDERCLAP => vec![damage_all(ctx, card, 4, 3), l("Apply 1 Vulnerable to ALL enemies.", "给予所有敌人1层易伤。")],
        TREMBLE => vec![apply_power("Vulnerable", "易伤", val(ctx, card, 3, 1))],
        TRUE_GRIT => vec![
            block(ctx, card, 7, 2),
            l("Exhaust 1 random card in your hand.", "随机消耗你手牌中的1张牌。"),
        ],
        TWIN_STRIKE => vec![damage_times(ctx, card, 5, 2, 2)],
        UNMOVABLE => vec![l("Block gained from cards is doubled.", "从卡牌获得的格挡翻倍。")],
        UNRELENTING => vec![
            damage(ctx, card, 12, 6),
            l("Your Attacks cost 0 this turn.", "本回合你的攻击牌耗能为0。"),
        ],
        UPPERCUT => vec![
            damage_exact(preview_damage(ctx, card, 13)),
            apply_power("Weak", "虚弱", if upgraded(ctx, card) { 2 } else { 1 }),
            apply_power("Vulnerable", "易伤", if upgraded(ctx, card) { 2 } else { 1 }),
        ],
        VICIOUS => vec![l(
            format!("Whenever you apply Vulnerable, gain {} Strength.", val(ctx, card, 1, 1)),
            format!("每当你给予易伤时，获得{}点力量。", val(ctx, card, 1, 1)),
        )],
        WHIRLWIND => vec![l(
            format!("Deal {} damage to ALL enemies X times.", preview_damage(ctx, card, val(ctx, card, 5, 3))),
            format!("对所有敌人造成{}点伤害X次。", preview_damage(ctx, card, val(ctx, card, 5, 3))),
        )],
        GIANT_ROCK => vec![damage(ctx, card, 16, 4)],
        _ => Vec::new(),
    }
}

fn l(eng: impl Into<String>, zhs: impl Into<String>) -> CardTextLine {
    CardTextLine {
        eng: eng.into(),
        zhs: zhs.into(),
    }
}

fn upgraded(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> bool {
    ctx.state
        .card(card)
        .map(|card| card.upgraded)
        .unwrap_or(false)
}

fn val(ctx: &CardTextCtx<'_>, card: CardInstanceId, base: i32, upgrade_delta: i32) -> i32 {
    base + if upgraded(ctx, card) {
        upgrade_delta
    } else {
        0
    }
}

fn damage(
    ctx: &CardTextCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
) -> CardTextLine {
    damage_exact(preview_damage(
        ctx,
        card,
        val(ctx, card, base, upgrade_delta),
    ))
}

fn damage_exact(amount: i32) -> CardTextLine {
    l(
        format!("Deal {amount} damage."),
        format!("造成{amount}点伤害。"),
    )
}

fn damage_all(
    ctx: &CardTextCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
) -> CardTextLine {
    damage_all_exact(preview_damage(
        ctx,
        card,
        val(ctx, card, base, upgrade_delta),
    ))
}

fn damage_all_exact(amount: i32) -> CardTextLine {
    l(
        format!("Deal {amount} damage to ALL enemies."),
        format!("对所有敌人造成{amount}点伤害。"),
    )
}

fn damage_times(
    ctx: &CardTextCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
    hits: u8,
) -> CardTextLine {
    let amount = preview_damage(ctx, card, val(ctx, card, base, upgrade_delta));
    if hits <= 1 {
        damage_exact(amount)
    } else {
        l(
            format!("Deal {amount} damage {hits} times."),
            format!("造成{amount}点伤害{hits}次。"),
        )
    }
}

fn damage_random_times(
    ctx: &CardTextCtx<'_>,
    card: CardInstanceId,
    base: i32,
    hits: u8,
) -> CardTextLine {
    let amount = preview_damage(ctx, card, base);
    l(
        format!("Deal {amount} damage to a random enemy {hits} times."),
        format!("对随机敌人造成{amount}点伤害{hits}次。"),
    )
}

fn block(
    ctx: &CardTextCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
) -> CardTextLine {
    let amount = block_amount(ctx, card, base, upgrade_delta);
    l(
        format!("Gain {amount} Block."),
        format!("获得{amount}点格挡。"),
    )
}

fn block_times(
    ctx: &CardTextCtx<'_>,
    card: CardInstanceId,
    base: i32,
    upgrade_delta: i32,
    times: u8,
) -> CardTextLine {
    let amount = block_amount(ctx, card, base, upgrade_delta);
    if times <= 1 {
        l(
            format!("Gain {amount} Block."),
            format!("获得{amount}点格挡。"),
        )
    } else {
        l(
            format!("Gain {amount} Block {times} times."),
            format!("获得{amount}点格挡{times}次。"),
        )
    }
}

fn block_amount(ctx: &CardTextCtx<'_>, card: CardInstanceId, base: i32, upgrade_delta: i32) -> i32 {
    let amount = val(ctx, card, base, upgrade_delta);
    if ctx.scope != CardTextScope::Hand {
        return amount;
    }
    let Some(target) = ctx.state.player_creature_id() else {
        return amount;
    };
    let calc = BlockCalc {
        source: Some(Source::Card(card)),
        target,
        base_amount: Decimal::from(amount),
        amount: Decimal::from(amount),
    };
    let (calc, _) = RulePipeline::modify_block(ctx.registry, ctx.state, calc);
    decimal_to_i32_trunc(calc.amount.max(Decimal::from(0)))
}

fn preview_damage(ctx: &CardTextCtx<'_>, card: CardInstanceId, amount: i32) -> i32 {
    if ctx.scope != CardTextScope::Hand {
        return amount;
    }
    let Some(target) = ctx.target.or_else(|| first_alive_enemy(ctx)) else {
        return amount;
    };
    let calc = DamageCalc {
        source: Some(Source::Card(card)),
        dealer: ctx.state.player_creature_id(),
        target,
        kind: DamageKind::Attack,
        base_amount: Decimal::from(amount),
        amount: Decimal::from(amount),
    };
    let (calc, _) = RulePipeline::modify_damage(ctx.registry, ctx.state, calc);
    decimal_to_i32_trunc(calc.amount.max(Decimal::from(0)))
}

fn first_alive_enemy(ctx: &CardTextCtx<'_>) -> Option<CreatureId> {
    ctx.state.alive_monster_ids().first().copied()
}

fn apply_power(eng: &str, zhs: &str, amount: i32) -> CardTextLine {
    l(
        format!("Apply {amount} {eng}."),
        format!("给予{amount}层{zhs}。"),
    )
}

fn gain_power(eng: &str, zhs: &str, amount: i32) -> CardTextLine {
    l(
        format!("Gain {amount} {eng}."),
        format!("获得{amount}点{zhs}。"),
    )
}

fn gain_energy(amount: i32) -> CardTextLine {
    l(
        format!("Gain {amount} Energy."),
        format!("获得{amount}点能量。"),
    )
}

fn draw(amount: i32) -> CardTextLine {
    l(
        format!("Draw {amount} {}.", plural(amount, "card", "cards")),
        format!("抽{amount}张牌。"),
    )
}

fn lose_hp(amount: i32) -> CardTextLine {
    l(
        format!("Lose {amount} HP."),
        format!("失去{amount}点生命。"),
    )
}

fn heal(amount: i32) -> CardTextLine {
    l(
        format!("Heal {amount} HP."),
        format!("治疗{amount}点生命。"),
    )
}

fn plural<'a>(amount: i32, one: &'a str, many: &'a str) -> &'a str {
    if amount == 1 {
        one
    } else {
        many
    }
}

fn player_block(ctx: &CardTextCtx<'_>) -> i32 {
    ctx.state
        .player_creature_id()
        .and_then(|id| ctx.state.creature(id))
        .map(|creature| creature.block)
        .unwrap_or(0)
}

fn target_has(ctx: &CardTextCtx<'_>, power: PowerId) -> bool {
    ctx.target
        .map(|target| ctx.state.power_amount(target, power) > 0)
        .unwrap_or(false)
}

fn exhaust_count(ctx: &CardTextCtx<'_>) -> i32 {
    ctx.state
        .combat()
        .map(|combat| combat.player.piles.exhaust.len() as i32)
        .unwrap_or(0)
}

fn previous_attacks_played(ctx: &CardTextCtx<'_>) -> i32 {
    ctx.state
        .combat()
        .map(|combat| combat.turn_stats.attacks_played.saturating_sub(1) as i32)
        .unwrap_or(0)
}

fn cards_exhausted_this_turn(ctx: &CardTextCtx<'_>) -> u32 {
    ctx.state
        .combat()
        .map(|combat| combat.turn_stats.cards_exhausted)
        .unwrap_or(0)
}

fn hp_lost_by_player_this_turn(ctx: &CardTextCtx<'_>) -> i32 {
    ctx.state
        .combat()
        .map(|combat| combat.turn_stats.hp_lost_by_player)
        .unwrap_or(0)
}

fn hp_loss_events_by_player(ctx: &CardTextCtx<'_>) -> u32 {
    ctx.state
        .combat()
        .map(|combat| combat.combat_stats.hp_loss_events_by_player)
        .unwrap_or(0)
}

fn hand_len(ctx: &CardTextCtx<'_>) -> usize {
    ctx.state
        .combat()
        .map(|combat| combat.player.piles.hand.len())
        .unwrap_or(0)
}

fn count_hand_type(ctx: &CardTextCtx<'_>, card_type: CardType) -> i32 {
    let Some(combat) = ctx.state.combat() else {
        return 0;
    };
    combat
        .player
        .piles
        .hand
        .iter()
        .filter(|card| {
            ctx.state
                .card(**card)
                .and_then(|card| ctx.registry.cards.get(card.def))
                .map(|def| def.card_type == card_type)
                .unwrap_or(false)
        })
        .count() as i32
}

fn count_all_tag(ctx: &CardTextCtx<'_>, tag: CardTag) -> i32 {
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

    use crate::content::cards::{BASH, RAMPAGE, STRIKE_IRONCLAD};
    use crate::content::powers::{STRENGTH, VULNERABLE};
    use crate::core::state::{CombatSetupCard, CombatSetupMonster};

    #[test]
    fn every_registered_ironclad_card_has_display_text() {
        let registry = StaticRegistry::standard();
        let state = GameState::single_player_test_combat(
            1,
            registry.cards.values().map(|def| CombatSetupCard {
                def: def.id,
                upgraded: false,
                costs: def.costs_for(false),
            }),
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            0,
        );
        let ctx = CardTextCtx {
            state: &state,
            registry: &registry,
            target: state.alive_monster_ids().first().copied(),
            scope: CardTextScope::Pile,
        };

        for card in state
            .combat()
            .unwrap()
            .cards
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            let text = describe_card(&ctx, card);
            assert!(!text.lines.is_empty(), "missing text for {:?}", card);
        }
    }

    #[test]
    fn hand_damage_preview_uses_source_and_target_modifiers() {
        let registry = StaticRegistry::standard();
        let mut state = GameState::single_player_test_combat(
            1,
            [CombatSetupCard {
                def: STRIKE_IRONCLAD,
                upgraded: false,
                costs: CardCosts::energy(1),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            1,
        );
        let player = state.player_creature_id().unwrap();
        let enemy = state.alive_monster_ids()[0];
        state
            .apply_power(player, STRENGTH, Decimal::from(2))
            .unwrap();
        state
            .apply_power(enemy, VULNERABLE, Decimal::from(1))
            .unwrap();
        let card = state.combat().unwrap().player.piles.hand[0];
        let ctx = CardTextCtx {
            state: &state,
            registry: &registry,
            target: Some(enemy),
            scope: CardTextScope::Hand,
        };

        let text = describe_card(&ctx, card);

        assert_eq!(text.lines[0].eng, "Deal 12 damage.");
    }

    #[test]
    fn rampage_text_uses_instance_counter() {
        let registry = StaticRegistry::standard();
        let mut state = GameState::single_player_test_combat(
            1,
            [CombatSetupCard {
                def: RAMPAGE,
                upgraded: false,
                costs: CardCosts::energy(1),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            1,
        );
        let enemy = state.alive_monster_ids()[0];
        let card = state.combat().unwrap().player.piles.hand[0];
        state
            .add_card_counter(card, CardCounter::DamageIncrease, 5)
            .unwrap();
        let ctx = CardTextCtx {
            state: &state,
            registry: &registry,
            target: Some(enemy),
            scope: CardTextScope::Hand,
        };

        let text = describe_card(&ctx, card);

        assert_eq!(text.lines[0].eng, "Deal 14 damage.");
    }

    #[test]
    fn pact_end_text_keeps_play_requirement_visible() {
        let registry = StaticRegistry::standard();
        let state = GameState::single_player_test_combat(
            1,
            [CombatSetupCard {
                def: PACTS_END,
                upgraded: false,
                costs: CardCosts::energy(0),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            1,
        );
        let card = state.combat().unwrap().player.piles.hand[0];
        let ctx = CardTextCtx {
            state: &state,
            registry: &registry,
            target: state.alive_monster_ids().first().copied(),
            scope: CardTextScope::Hand,
        };

        let text = describe_card(&ctx, card);

        assert!(text.lines[0].eng.contains("3 or more"));
    }

    #[test]
    fn bash_keywords_include_upgraded_dynamic_keywords() {
        let registry = StaticRegistry::standard();
        let mut state = GameState::single_player_test_combat(
            1,
            [CombatSetupCard {
                def: BASH,
                upgraded: false,
                costs: CardCosts::energy(2),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            1,
        );
        let card = state.combat().unwrap().player.piles.hand[0];
        state.card_mut(card).unwrap().flags.zero_cost_this_turn = true;
        let ctx = CardTextCtx {
            state: &state,
            registry: &registry,
            target: None,
            scope: CardTextScope::Hand,
        };

        let text = describe_card(&ctx, card);

        assert!(text.keywords.contains(&CardKeyword::FreeThisTurn));
    }
}
