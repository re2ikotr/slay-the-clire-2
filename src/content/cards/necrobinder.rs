use crate::core::ids::CardId;
use crate::registry::DefRegistry;

use super::{CardDef, CardPoolId};

pub fn register_necrobinder_cards(registry: &mut DefRegistry<CardId, CardDef>) {
    crate::content::generated_cards::register_pool_cards(registry, CardPoolId::Necrobinder);
}
