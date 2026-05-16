use std::collections::BTreeMap;

use crate::content::potions::PotionDef;
use crate::core::ids::PotionId;

#[derive(Default)]
pub struct PotionRegistry {
    defs: BTreeMap<PotionId, PotionDef>,
}

impl PotionRegistry {
    pub fn register(&mut self, def: PotionDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: PotionId) -> Option<&PotionDef> {
        self.defs.get(&id)
    }
}
