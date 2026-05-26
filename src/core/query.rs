use rust_decimal::Decimal;

use crate::core::effect::{DamageKind, OrbTrigger, Source};
use crate::core::ids::{CardInstanceId, CreatureId, OrbInstanceId, PlayerId, PowerId};
use crate::core::listener::ListenerRef;
use crate::core::state::{PileId, ResourceKind, Side};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    ModifyDamage(DamageCalc),
    ModifyBlock(BlockCalc),
    ModifyResourceCost(ResourceCostCalc),
    ModifyCardPlayCount(CardPlayCountCalc),
    ModifyCardPlayResultPile(CardPlayResultPileCalc),
    ModifyHandDraw(HandDrawCalc),
    ModifyHpLoss(HpLossCalc),
    ModifyUnblockedDamageTarget(UnblockedDamageTargetCalc),
    ModifyPowerAmount(PowerAmountCalc),
    ModifyOrbPassiveTriggerCount(OrbPassiveTriggerCountCalc),
    ModifyOrbValue(OrbValueCalc),
    ModifySummonAmount(SummonAmountCalc),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageCalc {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub kind: DamageKind,
    pub base_amount: Decimal,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockCalc {
    pub source: Option<Source>,
    pub target: CreatureId,
    pub base_amount: Decimal,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceCostCalc {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub resource: ResourceKind,
    pub base_cost: i32,
    pub cost: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardPlayCountCalc {
    pub card: CardInstanceId,
    pub target: Option<CreatureId>,
    pub base_count: i32,
    pub count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardPlayResultPileCalc {
    pub card: CardInstanceId,
    pub base_pile: PileId,
    pub pile: PileId,
    pub position: PilePosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilePosition {
    Top,
    Bottom,
    Random,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandDrawCalc {
    pub player: PlayerId,
    pub base_count: Decimal,
    pub count: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HpLossCalc {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub kind: DamageKind,
    pub base_amount: Decimal,
    pub amount: Decimal,
    pub phase: HpLossPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HpLossPhase {
    BeforeRedirect,
    AfterRedirect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnblockedDamageTargetCalc {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub original_target: CreatureId,
    pub target: CreatureId,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerAmountCalc {
    pub source: Option<Source>,
    pub giver: Option<CreatureId>,
    pub target: CreatureId,
    pub power: PowerId,
    pub base_amount: Decimal,
    pub amount: Decimal,
    pub phase: PowerAmountPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAmountPhase {
    Given,
    Received,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbPassiveTriggerCountCalc {
    pub player: PlayerId,
    pub orb: OrbInstanceId,
    pub trigger: OrbTrigger,
    pub base_count: i32,
    pub count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbValueCalc {
    pub player: PlayerId,
    pub orb: OrbInstanceId,
    pub base_amount: Decimal,
    pub amount: Decimal,
    pub kind: OrbValueKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbValueKind {
    Passive,
    Evoke,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummonAmountCalc {
    pub player: PlayerId,
    pub source: Option<Source>,
    pub base_amount: Decimal,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionQuery {
    pub kind: DecisionQueryKind,
    pub source: Option<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecisionQueryKind {
    ShouldPlay {
        card: CardInstanceId,
        target: Option<CreatureId>,
    },
    ShouldDraw {
        player: PlayerId,
        from_hand_draw: bool,
    },
    ShouldDie {
        creature: CreatureId,
    },
    ShouldRemoveCreatureAfterDeath {
        creature: CreatureId,
    },
    ShouldClearBlock {
        creature: CreatureId,
    },
    ShouldFlush {
        player: PlayerId,
    },
    ShouldStopCombatFromEnding,
    ShouldTakeExtraTurn {
        player: PlayerId,
    },
    ShouldStartTurn {
        side: Side,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Prevent {
        by: ListenerRef,
        reason: PreventReason,
    },
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreventReason {
    CannotPlay,
    CannotDraw,
    CannotDie,
    KeepsCreatureInCombat,
    NoValidTarget,
    InsufficientResource(ResourceKind),
    Custom(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifierLog {
    pub listener: ListenerRef,
    pub phase: ModifierPhase,
    pub before: Decimal,
    pub after: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardPlayResultPileModifierLog {
    pub listener: ListenerRef,
    pub before_pile: PileId,
    pub after_pile: PileId,
    pub before_position: PilePosition,
    pub after_position: PilePosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModifierPhase {
    Additive,
    Multiplicative,
    Capping,
    Replacement,
}
