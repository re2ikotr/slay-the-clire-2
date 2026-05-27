use super::{CardTextCtx, CardTextLine};
use crate::content::cards::GIANT_ROCK;
use crate::core::ids::CardInstanceId;

pub(super) fn describe_lines(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardTextLine> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    match card_state.def {
        GIANT_ROCK => {
            let amount = if card_state.upgraded { 20 } else { 16 };
            vec![CardTextLine {
                eng: format!("Deal {amount} damage."),
                zhs: format!("造成{amount}点伤害。"),
            }]
        }
        _ => crate::content::generated_cards::describe_lines(ctx, card),
    }
}
