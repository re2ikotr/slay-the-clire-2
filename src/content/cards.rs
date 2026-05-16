use crate::core::effect::Effect;
use crate::core::ids::{CardId, CardInstanceId, CreatureId, LocKey};
use crate::core::state::{CardCosts, GameState};

pub type CardPlayFn =
    for<'a> fn(&CardPlayCtx<'a>, CardInstanceId, Option<CreatureId>) -> Vec<Effect>;

#[derive(Clone)]
pub struct CardDef {
    pub id: CardId,
    pub loc_key: LocKey,
    pub card_type: CardType,
    pub rarity: CardRarity,
    pub target: TargetType,
    pub base_costs: CardCosts,
    pub play: CardPlayFn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardType {
    Attack,
    Skill,
    Power,
    Status,
    Curse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardRarity {
    Basic,
    Common,
    Uncommon,
    Rare,
    Special,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetType {
    None,
    Enemy,
    AllEnemies,
    SelfTarget,
    AnyCreature,
}

pub struct CardPlayCtx<'a> {
    pub state: &'a GameState,
}

pub fn no_card_effect(
    _ctx: &CardPlayCtx<'_>,
    _card: CardInstanceId,
    _target: Option<CreatureId>,
) -> Vec<Effect> {
    Vec::new()
}
