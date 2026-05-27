use super::{CardTextCtx, CardTextLine};
use crate::core::ids::CardInstanceId;

pub(super) fn describe_lines(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardTextLine> {
    crate::content::generated_cards::describe_lines(ctx, card)
}
