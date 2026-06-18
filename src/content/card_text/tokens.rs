use super::{CardTextCtx, CardTextLine};
use crate::content::cards::GIANT_ROCK;
use crate::content::generated_cards::{
    DISINTEGRATION, FUEL, LUMINESCE, MIND_ROT, MINION_DIVE_BOMB, MINION_SACRIFICE, MINION_STRIKE,
    SHIV, SLOTH, SOUL, SOVEREIGN_BLADE, SWEEPING_GAZE, WASTE_AWAY,
};
use crate::core::ids::CardInstanceId;

pub(super) fn describe_lines(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardTextLine> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    let upgraded = card_state.upgraded;
    match card_state.def {
        // GIANT_ROCK is hand-coded inside ironclad.rs but registered into the
        // Token pool, so it falls through here for text resolution too.
        GIANT_ROCK => {
            let amount = if upgraded { 20 } else { 16 };
            vec![l(
                format!("Deal {amount} damage."),
                format!("造成{amount}点伤害。"),
            )]
        }

        // ---- Playable tokens ----
        FUEL => {
            let cards = if upgraded { 2 } else { 1 };
            let cards_zhs = if upgraded { "2张牌" } else { "1张牌" };
            vec![
                l("Gain [energy:1].", "获得[energy:1]。"),
                l(
                    format!("Draw {cards} card{}.", if cards == 1 { "" } else { "s" }),
                    format!("抽{cards_zhs}。"),
                ),
            ]
        }
        LUMINESCE => {
            let amount = if upgraded { 3 } else { 2 };
            vec![l(
                format!("Gain [energy:{amount}]."),
                format!("获得[energy:{amount}]。"),
            )]
        }
        MINION_DIVE_BOMB => {
            let amount = if upgraded { 16 } else { 13 };
            vec![l(
                format!("Deal {amount} damage."),
                format!("造成{amount}点伤害。"),
            )]
        }
        MINION_SACRIFICE => {
            let amount = if upgraded { 12 } else { 9 };
            vec![l(
                format!("Gain {amount} [gold]Block[/gold]."),
                format!("获得{amount}点[gold]格挡[/gold]。"),
            )]
        }
        MINION_STRIKE => {
            let amount = if upgraded { 9 } else { 6 };
            vec![
                l(
                    format!("Deal {amount} damage."),
                    format!("造成{amount}点伤害。"),
                ),
                l("Draw 1 card.", "抽1张牌。"),
            ]
        }
        SHIV => {
            let amount = if upgraded { 6 } else { 4 };
            // Note: the original catalog text said "to ALL enemies" but the
            // play behavior is a single-target attack. We keep the text in
            // sync with the actual effect; the C# source uses single-target
            // unless the FanOfKnives power is active, which this engine does
            // not model yet.
            vec![l(
                format!("Deal {amount} damage."),
                format!("造成{amount}点伤害。"),
            )]
        }
        SOUL => {
            let amount = if upgraded { 3 } else { 2 };
            vec![l(
                format!("Draw {amount} cards."),
                format!("抽{amount}张牌。"),
            )]
        }
        SOVEREIGN_BLADE => vec![l("Deal 10 damage.", "造成10点伤害。")],
        SWEEPING_GAZE => {
            let amount = if upgraded { 15 } else { 10 };
            vec![l(
                format!("[gold]Osty[/gold] deals {amount} damage to a random enemy."),
                format!("[gold]奥斯提[/gold]对随机一名敌人造成{amount}点伤害。"),
            )]
        }

        // ---- Status tokens ----
        DISINTEGRATION => vec![l(
            "At the end of your turn, take 6 damage.",
            "在你的回合结束时，受到6点伤害。",
        )],
        MIND_ROT => vec![l("Draw 1 fewer card each turn.", "每回合少抽1张牌。")],
        SLOTH => vec![l(
            "You cannot play more than 3 cards each turn.",
            "你在每个回合不能打出超过3张牌。",
        )],
        WASTE_AWAY => vec![l(
            "Gain 1 less [energy:1] per turn.",
            "每回合失去1点[energy:1]。",
        )],

        _ => Vec::new(),
    }
}

fn l(eng: impl Into<String>, zhs: impl Into<String>) -> CardTextLine {
    CardTextLine {
        eng: eng.into(),
        zhs: zhs.into(),
    }
}
