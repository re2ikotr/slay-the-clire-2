use crate::core::effect::Effect;
use crate::core::ids::{CreatureId, LocKey, MonsterId};
use crate::core::rules::RuleCtx;

pub type MonsterIntentFn = for<'a> fn(&RuleCtx<'a>, CreatureId) -> MonsterIntent;
pub type MonsterActFn = for<'a> fn(&RuleCtx<'a>, CreatureId) -> Vec<Effect>;

#[derive(Clone)]
pub struct MonsterDef {
    pub id: MonsterId,
    pub loc_key: LocKey,
    pub max_hp: rust_decimal::Decimal,
    pub intent: MonsterIntentFn,
    pub act: MonsterActFn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonsterIntent {
    Attack { amount: rust_decimal::Decimal },
    Block { amount: rust_decimal::Decimal },
    Debuff,
    Unknown,
}
