use std::collections::BTreeMap;

use crate::content::relics::RelicDef;
use crate::core::ids::RelicId;

#[derive(Default)]
pub struct RelicRegistry {
    defs: BTreeMap<RelicId, RelicDef>,
}

impl RelicRegistry {
    pub fn register(&mut self, def: RelicDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: RelicId) -> Option<&RelicDef> {
        self.defs.get(&id)
    }
}
