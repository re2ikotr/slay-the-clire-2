use crate::core::ids::{LocKey, MonsterId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterDef {
    pub id: &'static str,
    pub loc_key: LocKey,
    pub monsters: Vec<MonsterSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterSlot {
    pub monster: MonsterId,
    pub count: u8,
}
