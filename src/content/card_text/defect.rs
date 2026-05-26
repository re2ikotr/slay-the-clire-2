use super::{CardTextCtx, CardTextLine};
use crate::core::ids::CardInstanceId;

pub(super) fn describe_lines(_ctx: &CardTextCtx<'_>, _card: CardInstanceId) -> Vec<CardTextLine> {
    Vec::new()
}
