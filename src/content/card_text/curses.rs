use super::{CardTextCtx, CardTextLine};
use crate::content::generated_cards::{
    ASCENDERS_BANE, BAD_LUCK, CLUMSY, CURSE_OF_THE_BELL, DEBT, DECAY, DOUBT, ENTHRALLED, FOLLY,
    GREED, GUILTY, INJURY, NORMALITY, POOR_SLEEP, REGRET, SHAME, SPORE_MIND, WRITHE,
};
use crate::core::ids::CardInstanceId;

pub(super) fn describe_lines(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardTextLine> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    match card_state.def {
        // Pure shells with no in-game text. We still emit a single empty line
        // so callers iterating "every card has at least one text line" hold;
        // this matches the legacy catalog behavior, which produced one empty
        // `CardTextLine` from a `description_eng: ""` entry.
        ASCENDERS_BANE | CLUMSY | CURSE_OF_THE_BELL | FOLLY | GREED | INJURY | POOR_SLEEP
        | SPORE_MIND | WRITHE => vec![l("", "")],

        DEBT => vec![l(
            "At the end of your turn, if this is in your [gold]Hand[/gold], lose 10 [gold]Gold[/gold].",
            "在你的回合结束时，如果这张牌在你的[gold]手牌[/gold]中，则失去10[gold]金币[/gold]。",
        )],
        BAD_LUCK => vec![l(
            "At the end of your turn, if this is in your Hand, lose 13 HP.",
            "在你的回合结束时，如果这张牌在你的手牌中，则失去13点生命。",
        )],
        DECAY => vec![l(
            "At the end of your turn, if this is in your [gold]Hand[/gold], take 2 damage.",
            "在你的回合结束时，如果这张牌在你的[gold]手牌[/gold]中, 你受到2点伤害。",
        )],
        DOUBT => vec![l(
            "At the end of your turn, if this is in your [gold]Hand[/gold], gain 1 [gold]Weak[/gold].",
            "在你的回合结束时，如果这张牌在你的[gold]手牌[/gold]中，获得1层[gold]虚弱[/gold]。",
        )],
        SHAME => vec![l(
            "At the end of your turn, if this is in your [gold]Hand[/gold], gain 1 [gold]Frail[/gold].",
            "在你的回合结束时，如果这张牌在你的[gold]手牌[/gold]中，则获得1层[gold]脆弱[/gold]。",
        )],
        REGRET => vec![l(
            "At the end of your turn, if this is in your [gold]Hand[/gold], lose 1 HP for each card in your [gold]Hand[/gold].",
            "在你的回合结束时，如果这张牌在你的[gold]手牌[/gold]中，失去相当于[gold]手牌[/gold]数量的生命。",
        )],
        GUILTY => vec![l(
            "Removed from your [gold]Deck[/gold] after 5 combats.",
            "在5场战斗后从你的[gold]牌组[/gold]中移除。",
        )],
        NORMALITY => vec![l(
            "You cannot play more than 3 cards this turn.",
            "你在本回合不能打出超过3张牌。",
        )],
        ENTHRALLED => vec![l(
            "If this is in your [gold]Hand[/gold], it must be played before other cards.",
            "如果这张牌在你的[gold]手牌[/gold]中，你必须优先打出这张牌。",
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
