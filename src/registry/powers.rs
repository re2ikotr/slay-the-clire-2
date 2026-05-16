use std::collections::BTreeMap;

use crate::content::powers::PowerDef;
use crate::core::ids::PowerId;

#[derive(Default)]
pub struct PowerRegistry {
    defs: BTreeMap<PowerId, PowerDef>,
}

impl PowerRegistry {
    pub fn register(&mut self, def: PowerDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: PowerId) -> Option<&PowerDef> {
        self.defs.get(&id)
    }
}
