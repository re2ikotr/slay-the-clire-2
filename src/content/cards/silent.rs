use crate::core::ids::CardId;
use crate::registry::DefRegistry;

use super::CardDef;

pub fn register_silent_cards(_registry: &mut DefRegistry<CardId, CardDef>) {}
