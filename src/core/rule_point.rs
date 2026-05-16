use crate::core::query::{DecisionQueryKind, ModifierPhase};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RulePoint {
    Event(EventRulePoint),
    Query(QueryRulePoint),
    Decision(DecisionQueryKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventRulePoint {
    CombatStarted,
    TurnStarted,
    TurnEnded,
    CardDrawn,
    CardPlayStarted,
    CardPlayed,
    DamageDealt,
    BlockGained,
    PowerApplied,
    ResourceSpent,
    ResourceGained,
    CreatureHpChanged,
    DeathPrevented,
    CreatureDied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QueryRulePoint {
    ModifyDamage(ModifierPhase),
    ModifyBlock(ModifierPhase),
    ModifyResourceCost,
    ModifyCardPlayCount,
    ModifyCardPlayResultPile,
    ModifyHandDraw,
    ModifyHpLostBeforeRedirect,
    ModifyUnblockedDamageTarget,
    ModifyHpLostAfterRedirect,
}
