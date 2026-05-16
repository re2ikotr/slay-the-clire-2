use std::collections::BTreeMap;

use crate::content::cards::CardDef;
use crate::core::ids::CardId;

#[derive(Default)]
pub struct CardRegistry {
    defs: BTreeMap<CardId, CardDef>,
}

impl CardRegistry {
    pub fn register(&mut self, def: CardDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: CardId) -> Option<&CardDef> {
        self.defs.get(&id)
    }

    pub fn contains(&self, id: CardId) -> bool {
        self.defs.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}
