use crate::core::effect::Effect;
use crate::core::ids::{CreatureId, LocKey, PotionId, PotionInstanceId};
use crate::core::rules::RuleCtx;

pub type PotionUseFn =
    for<'a> fn(&RuleCtx<'a>, PotionInstanceId, Option<CreatureId>) -> Vec<Effect>;

#[derive(Clone)]
pub struct PotionDef {
    pub id: PotionId,
    pub loc_key: LocKey,
    pub target: PotionTarget,
    pub use_potion: PotionUseFn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PotionTarget {
    None,
    Enemy,
    AnyCreature,
}
