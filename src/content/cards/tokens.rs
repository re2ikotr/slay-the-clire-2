use crate::core::ids::CardId;
use crate::registry::DefRegistry;

use super::{CardDef, CardPoolId};

pub fn register_token_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    registry.register(super::ironclad::giant_rock_def());
    crate::content::generated_cards::register_pool_cards(registry, CardPoolId::Token);
}
