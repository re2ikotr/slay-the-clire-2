use std::collections::BTreeMap;

use crate::content::cards::CardDef;
use crate::content::monsters::MonsterDef;
use crate::content::potions::PotionDef;
use crate::content::powers::PowerDef;
use crate::content::relics::RelicDef;
use crate::core::ids::{CardId, MonsterId, PotionId, PowerId, RelicId};

pub trait RegistryDef {
    type Id: Copy + Ord;

    fn registry_id(&self) -> Self::Id;
}

pub struct DefRegistry<Id, Def> {
    defs: BTreeMap<Id, Def>,
}

impl<Id, Def> Default for DefRegistry<Id, Def> {
    fn default() -> Self {
        Self {
            defs: BTreeMap::new(),
        }
    }
}

impl<Id, Def> DefRegistry<Id, Def>
where
    Id: Copy + Ord,
    Def: RegistryDef<Id = Id>,
{
    pub fn register(&mut self, def: Def) {
        self.defs.insert(def.registry_id(), def);
    }
}

impl<Id, Def> DefRegistry<Id, Def>
where
    Id: Copy + Ord,
{
    pub fn get(&self, id: Id) -> Option<&Def> {
        self.defs.get(&id)
    }

    pub fn contains(&self, id: Id) -> bool {
        self.defs.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

impl RegistryDef for CardDef {
    type Id = CardId;

    fn registry_id(&self) -> Self::Id {
        self.id
    }
}

impl RegistryDef for PowerDef {
    type Id = PowerId;

    fn registry_id(&self) -> Self::Id {
        self.id
    }
}

impl RegistryDef for RelicDef {
    type Id = RelicId;

    fn registry_id(&self) -> Self::Id {
        self.id
    }
}

impl RegistryDef for PotionDef {
    type Id = PotionId;

    fn registry_id(&self) -> Self::Id {
        self.id
    }
}

impl RegistryDef for MonsterDef {
    type Id = MonsterId;

    fn registry_id(&self) -> Self::Id {
        self.id
    }
}

pub struct StaticRegistry {
    pub cards: DefRegistry<CardId, CardDef>,
    pub powers: DefRegistry<PowerId, PowerDef>,
    pub relics: DefRegistry<RelicId, RelicDef>,
    pub potions: DefRegistry<PotionId, PotionDef>,
    pub monsters: DefRegistry<MonsterId, MonsterDef>,
}

impl StaticRegistry {
    pub fn empty() -> Self {
        Self {
            cards: DefRegistry::default(),
            powers: DefRegistry::default(),
            relics: DefRegistry::default(),
            potions: DefRegistry::default(),
            monsters: DefRegistry::default(),
        }
    }

    pub fn standard() -> Self {
        let mut registry = Self::empty();
        registry
            .cards
            .register(crate::content::cards::strike_ironclad());
        registry
            .cards
            .register(crate::content::cards::defend_ironclad());
        registry.powers.register(crate::content::powers::strength());
        registry
            .monsters
            .register(crate::content::monsters::nibbit());
        registry
    }
}

impl Default for StaticRegistry {
    fn default() -> Self {
        Self::standard()
    }
}
