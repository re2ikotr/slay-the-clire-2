use std::collections::BTreeMap;

use crate::content::monsters::MonsterDef;
use crate::core::ids::MonsterId;

#[derive(Default)]
pub struct MonsterRegistry {
    defs: BTreeMap<MonsterId, MonsterDef>,
}

impl MonsterRegistry {
    pub fn register(&mut self, def: MonsterDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: MonsterId) -> Option<&MonsterDef> {
        self.defs.get(&id)
    }
}
