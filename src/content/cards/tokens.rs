use crate::core::ids::CardId;
use crate::registry::DefRegistry;

use super::CardDef;

pub fn register_token_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    registry.register(super::ironclad::giant_rock_def());
}
