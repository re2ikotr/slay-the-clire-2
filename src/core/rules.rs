use rust_decimal::Decimal;

use crate::content::cards::CardRules;
use crate::content::monsters::MonsterRules;
use crate::content::orbs::OrbRules;
use crate::content::potions::PotionRules;
use crate::content::powers::PowerRules;
use crate::content::relics::RelicRules;
use crate::core::effect::{Effect, Source};
use crate::core::event::Event;
use crate::core::ids::{
    CardInstanceId, CreatureId, OrbInstanceId, PlayerId, PotionInstanceId, PowerInstanceId,
    RelicInstanceId,
};
use crate::core::listener::{collect_combat_listeners, ListenerRef, ListenerScope};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, CardPlayResultPileModifierLog, DamageCalc, Decision,
    DecisionQuery, DecisionQueryKind, HpLossCalc, ModifierLog, ModifierPhase,
    OrbPassiveTriggerCountCalc, OrbValueCalc, PowerAmountCalc, PreventReason, ResourceCostCalc,
    SummonAmountCalc, UnblockedDamageTargetCalc,
};
use crate::core::state::GameState;
use crate::registry::StaticRegistry;

#[derive(Default)]
pub struct RulePipeline;

impl RulePipeline {
    pub fn event_listeners(state: &GameState, event: &Event) -> Vec<ListenerRef> {
        collect_combat_listeners(state, event_listener_scope(event))
    }

    pub fn notify_listener(
        registry: &StaticRegistry,
        state: &GameState,
        listener: ListenerRef,
        event: &Event,
    ) -> Vec<Effect> {
        let ctx = RuleCtx {
            state,
            registry,
            listener: Some(listener),
        };
        dispatch_event(registry, state, listener, &ctx, event)
    }

    pub fn notify(registry: &StaticRegistry, state: &GameState, event: &Event) -> Vec<Effect> {
        let mut out = Vec::new();

        for listener in Self::event_listeners(state, event) {
            out.extend(Self::notify_listener(registry, state, listener, event));
        }

        out
    }

    pub fn modify_damage(
        registry: &StaticRegistry,
        state: &GameState,
        calc: DamageCalc,
    ) -> (DamageCalc, Vec<ModifierLog>) {
        let mut calc = calc;
        let mut logs = Vec::new();

        for phase in [
            ModifierPhase::Additive,
            ModifierPhase::Multiplicative,
            ModifierPhase::Capping,
        ] {
            let listeners = collect_combat_listeners(state, damage_listener_scope(&calc));
            for listener in listeners.iter().copied() {
                let before = calc.amount;
                let ctx = RuleCtx {
                    state,
                    registry,
                    listener: Some(listener),
                };
                calc = dispatch_modify_damage(registry, state, listener, &ctx, phase, calc);
                if calc.amount != before {
                    logs.push(ModifierLog {
                        listener,
                        phase,
                        before,
                        after: calc.amount,
                    });
                }
            }
        }

        (calc, logs)
    }

    pub fn modify_block(
        registry: &StaticRegistry,
        state: &GameState,
        calc: BlockCalc,
    ) -> (BlockCalc, Vec<ModifierLog>) {
        let mut calc = calc;
        let mut logs = Vec::new();

        for phase in [ModifierPhase::Additive, ModifierPhase::Multiplicative] {
            let listeners = collect_combat_listeners(state, block_listener_scope(&calc));
            for listener in listeners.iter().copied() {
                let before = calc.amount;
                let ctx = RuleCtx {
                    state,
                    registry,
                    listener: Some(listener),
                };
                calc = dispatch_modify_block(registry, state, listener, &ctx, phase, calc);
                if calc.amount != before {
                    logs.push(ModifierLog {
                        listener,
                        phase,
                        before,
                        after: calc.amount,
                    });
                }
            }
        }

        (calc, logs)
    }

    pub fn modify_hp_loss(
        registry: &StaticRegistry,
        state: &GameState,
        calc: HpLossCalc,
    ) -> (HpLossCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, hp_loss_listener_scope(&calc));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = calc.amount;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_hp_loss(registry, state, listener, &ctx, calc);
            if calc.amount != before {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before,
                    after: calc.amount,
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_unblocked_damage_target(
        registry: &StaticRegistry,
        state: &GameState,
        calc: UnblockedDamageTargetCalc,
    ) -> (UnblockedDamageTargetCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, unblocked_target_listener_scope(&calc));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = Decimal::from(calc.target.get());
            let before_target = calc.target;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_unblocked_damage_target(registry, state, listener, &ctx, calc);
            if calc.target != before_target {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before,
                    after: Decimal::from(calc.target.get()),
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_resource_cost(
        registry: &StaticRegistry,
        state: &GameState,
        calc: ResourceCostCalc,
    ) -> (ResourceCostCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, resource_cost_listener_scope(state, &calc));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = calc.cost;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_resource_cost(registry, state, listener, &ctx, calc);
            if calc.cost != before {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before: Decimal::from(before),
                    after: Decimal::from(calc.cost),
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_card_play_result_pile(
        registry: &StaticRegistry,
        state: &GameState,
        calc: CardPlayResultPileCalc,
    ) -> (CardPlayResultPileCalc, Vec<CardPlayResultPileModifierLog>) {
        let listeners =
            collect_combat_listeners(state, card_result_pile_listener_scope(state, &calc));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before_pile = calc.pile;
            let before_position = calc.position;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_card_play_result_pile(registry, state, listener, &ctx, calc);
            if calc.pile != before_pile || calc.position != before_position {
                logs.push(CardPlayResultPileModifierLog {
                    listener,
                    before_pile,
                    after_pile: calc.pile,
                    before_position,
                    after_position: calc.position,
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_power_amount(
        registry: &StaticRegistry,
        state: &GameState,
        calc: PowerAmountCalc,
    ) -> (PowerAmountCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, power_amount_listener_scope(&calc));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = calc.amount;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_power_amount(registry, state, listener, &ctx, calc);
            if calc.amount != before {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before,
                    after: calc.amount,
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_orb_passive_trigger_count(
        registry: &StaticRegistry,
        state: &GameState,
        calc: OrbPassiveTriggerCountCalc,
    ) -> (OrbPassiveTriggerCountCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, player_listener_scope(state, calc.player));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = calc.count;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_orb_passive_trigger_count(registry, state, listener, &ctx, calc);
            if calc.count != before {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before: Decimal::from(before),
                    after: Decimal::from(calc.count),
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_orb_value(
        registry: &StaticRegistry,
        state: &GameState,
        calc: OrbValueCalc,
    ) -> (OrbValueCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, player_listener_scope(state, calc.player));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = calc.amount;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_orb_value(registry, state, listener, &ctx, calc);
            if calc.amount != before {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before,
                    after: calc.amount,
                });
            }
        }

        (calc, logs)
    }

    pub fn modify_summon_amount(
        registry: &StaticRegistry,
        state: &GameState,
        calc: SummonAmountCalc,
    ) -> (SummonAmountCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, player_listener_scope(state, calc.player));
        let mut calc = calc;
        let mut logs = Vec::new();

        for listener in listeners {
            let before = calc.amount;
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            calc = dispatch_modify_summon_amount(registry, state, listener, &ctx, calc);
            if calc.amount != before {
                logs.push(ModifierLog {
                    listener,
                    phase: ModifierPhase::Replacement,
                    before,
                    after: calc.amount,
                });
            }
        }

        (calc, logs)
    }

    pub fn decide(registry: &StaticRegistry, state: &GameState, query: DecisionQuery) -> Decision {
        let scope = decision_listener_scope(state, &query);
        for listener in collect_combat_listeners(state, scope) {
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            match dispatch_decision(registry, state, listener, &ctx, &query) {
                Decision::Allow => {}
                Decision::Prevent { reason, .. } => {
                    return Decision::Prevent {
                        by: listener,
                        reason,
                    };
                }
            }
        }

        Decision::Allow
    }

    pub fn should_play(
        registry: &StaticRegistry,
        state: &GameState,
        card: CardInstanceId,
        target: Option<CreatureId>,
    ) -> Decision {
        Self::decide(
            registry,
            state,
            DecisionQuery {
                kind: DecisionQueryKind::ShouldPlay { card, target },
                source: Some(Source::Card(card)),
            },
        )
    }

    pub fn should_draw(
        registry: &StaticRegistry,
        state: &GameState,
        player: crate::core::ids::PlayerId,
        from_hand_draw: bool,
    ) -> Decision {
        Self::decide(
            registry,
            state,
            DecisionQuery {
                kind: DecisionQueryKind::ShouldDraw {
                    player,
                    from_hand_draw,
                },
                source: None,
            },
        )
    }

    pub fn should_clear_block(
        registry: &StaticRegistry,
        state: &GameState,
        creature: CreatureId,
    ) -> Decision {
        Self::decide(
            registry,
            state,
            DecisionQuery {
                kind: DecisionQueryKind::ShouldClearBlock { creature },
                source: None,
            },
        )
    }

    pub fn should_die(
        registry: &StaticRegistry,
        state: &GameState,
        creature: CreatureId,
    ) -> Decision {
        Self::decide(
            registry,
            state,
            DecisionQuery {
                kind: DecisionQueryKind::ShouldDie { creature },
                source: None,
            },
        )
    }

    pub fn should_remove_creature_after_death(
        registry: &StaticRegistry,
        state: &GameState,
        creature: CreatureId,
    ) -> Decision {
        Self::decide(
            registry,
            state,
            DecisionQuery {
                kind: DecisionQueryKind::ShouldRemoveCreatureAfterDeath { creature },
                source: None,
            },
        )
    }
}

pub struct RuleCtx<'a> {
    pub state: &'a GameState,
    pub registry: &'a StaticRegistry,
    pub listener: Option<ListenerRef>,
}

fn dispatch_event(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    event: &Event,
) -> Vec<Effect> {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_event(ctx, event))
        .unwrap_or_default()
}

fn dispatch_modify_damage(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    phase: ModifierPhase,
    calc: DamageCalc,
) -> DamageCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_damage(ctx, phase, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_block(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    phase: ModifierPhase,
    calc: BlockCalc,
) -> BlockCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_block(ctx, phase, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_hp_loss(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: HpLossCalc,
) -> HpLossCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_hp_loss(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_unblocked_damage_target(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: UnblockedDamageTargetCalc,
) -> UnblockedDamageTargetCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_unblocked_damage_target(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_resource_cost(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: ResourceCostCalc,
) -> ResourceCostCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_resource_cost(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_card_play_result_pile(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: CardPlayResultPileCalc,
) -> CardPlayResultPileCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_card_play_result_pile(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_power_amount(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: PowerAmountCalc,
) -> PowerAmountCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_power_amount(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_orb_passive_trigger_count(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: OrbPassiveTriggerCountCalc,
) -> OrbPassiveTriggerCountCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_orb_passive_trigger_count(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_orb_value(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: OrbValueCalc,
) -> OrbValueCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_orb_value(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_modify_summon_amount(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: SummonAmountCalc,
) -> SummonAmountCalc {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_modify_summon_amount(ctx, calc.clone()))
        .unwrap_or(calc)
}

fn dispatch_decision(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    query: &DecisionQuery,
) -> Decision {
    ListenerRulesRef::for_listener(registry, state, listener)
        .map(|rules| rules.dispatch_decision(ctx, query))
        .unwrap_or(Decision::Allow)
}

#[derive(Clone, Copy)]
enum ListenerRulesRef<'a> {
    Power(PowerInstanceId, &'a PowerRules),
    Relic(RelicInstanceId, &'a RelicRules),
    Potion(PotionInstanceId, &'a PotionRules),
    Orb(OrbInstanceId, &'a OrbRules),
    Monster(CreatureId, &'a MonsterRules),
    Card(CardInstanceId, &'a CardRules),
}

impl<'a> ListenerRulesRef<'a> {
    fn for_listener(
        registry: &'a StaticRegistry,
        state: &GameState,
        listener: ListenerRef,
    ) -> Option<Self> {
        match listener {
            ListenerRef::Power(id) => {
                let def = state
                    .combat()
                    .and_then(|combat| combat.powers.get(&id))
                    .and_then(|instance| registry.powers.get(instance.def))?;
                Some(Self::Power(id, &def.rules))
            }
            ListenerRef::Relic(id) => {
                let def = state
                    .combat()
                    .and_then(|combat| combat.relics.get(&id))
                    .and_then(|instance| registry.relics.get(instance.def))?;
                Some(Self::Relic(id, &def.rules))
            }
            ListenerRef::Potion(id) => {
                let def = state
                    .combat()
                    .and_then(|combat| combat.potions.get(&id))
                    .and_then(|instance| registry.potions.get(instance.def))?;
                Some(Self::Potion(id, &def.rules))
            }
            ListenerRef::Orb(id) => {
                let def = state
                    .combat()
                    .and_then(|combat| combat.orbs.get(&id))
                    .and_then(|instance| registry.orbs.get(instance.def))?;
                Some(Self::Orb(id, &def.rules))
            }
            ListenerRef::Monster(id) => {
                let def = state
                    .creature(id)
                    .and_then(|creature| creature.model)
                    .and_then(|model| registry.monsters.get(model))?;
                Some(Self::Monster(id, &def.rules))
            }
            ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
                let def = card_def(registry, state, id)?;
                Some(Self::Card(id, &def.rules))
            }
            ListenerRef::Modifier(_) => None,
        }
    }

    fn dispatch_event(self, ctx: &RuleCtx<'_>, event: &Event) -> Vec<Effect> {
        match self {
            Self::Power(id, rules) => apply_event(ctx, id, rules.on_event, event),
            Self::Relic(id, rules) => apply_event(ctx, id, rules.on_event, event),
            Self::Potion(id, rules) => apply_event(ctx, id, rules.on_event, event),
            Self::Orb(_, _) => Vec::new(),
            Self::Monster(id, rules) => apply_event(ctx, id, rules.on_event, event),
            Self::Card(id, rules) => apply_event(ctx, id, rules.on_event, event),
        }
    }

    fn dispatch_modify_damage(
        self,
        ctx: &RuleCtx<'_>,
        phase: ModifierPhase,
        calc: DamageCalc,
    ) -> DamageCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.damage_rule(phase), calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.damage_rule(phase), calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.damage_rule(phase), calc),
            Self::Orb(_, _) => calc,
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.damage_rule(phase), calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.damage_rule(phase), calc),
        }
    }

    fn dispatch_modify_block(
        self,
        ctx: &RuleCtx<'_>,
        phase: ModifierPhase,
        calc: BlockCalc,
    ) -> BlockCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.block_rule(phase), calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.block_rule(phase), calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.block_rule(phase), calc),
            Self::Orb(_, _) => calc,
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.block_rule(phase), calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.block_rule(phase), calc),
        }
    }

    fn dispatch_modify_hp_loss(self, ctx: &RuleCtx<'_>, calc: HpLossCalc) -> HpLossCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.modify_hp_loss, calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.modify_hp_loss, calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.modify_hp_loss, calc),
            Self::Orb(_, _) => calc,
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.modify_hp_loss, calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.modify_hp_loss, calc),
        }
    }

    fn dispatch_modify_unblocked_damage_target(
        self,
        ctx: &RuleCtx<'_>,
        calc: UnblockedDamageTargetCalc,
    ) -> UnblockedDamageTargetCalc {
        match self {
            Self::Power(id, rules) => {
                apply_calc(ctx, id, rules.modify_unblocked_damage_target, calc)
            }
            Self::Relic(id, rules) => {
                apply_calc(ctx, id, rules.modify_unblocked_damage_target, calc)
            }
            Self::Potion(id, rules) => {
                apply_calc(ctx, id, rules.modify_unblocked_damage_target, calc)
            }
            Self::Orb(_, _) => calc,
            Self::Monster(id, rules) => {
                apply_calc(ctx, id, rules.modify_unblocked_damage_target, calc)
            }
            Self::Card(id, rules) => {
                apply_calc(ctx, id, rules.modify_unblocked_damage_target, calc)
            }
        }
    }

    fn dispatch_modify_resource_cost(
        self,
        ctx: &RuleCtx<'_>,
        calc: ResourceCostCalc,
    ) -> ResourceCostCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.modify_resource_cost, calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.modify_resource_cost, calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.modify_resource_cost, calc),
            Self::Orb(_, _) => calc,
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.modify_resource_cost, calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.modify_resource_cost, calc),
        }
    }

    fn dispatch_modify_card_play_result_pile(
        self,
        ctx: &RuleCtx<'_>,
        calc: CardPlayResultPileCalc,
    ) -> CardPlayResultPileCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.modify_card_play_result_pile, calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.modify_card_play_result_pile, calc),
            Self::Potion(id, rules) => {
                apply_calc(ctx, id, rules.modify_card_play_result_pile, calc)
            }
            Self::Orb(_, _) => calc,
            Self::Monster(id, rules) => {
                apply_calc(ctx, id, rules.modify_card_play_result_pile, calc)
            }
            Self::Card(id, rules) => apply_calc(ctx, id, rules.modify_card_play_result_pile, calc),
        }
    }

    fn dispatch_modify_power_amount(
        self,
        ctx: &RuleCtx<'_>,
        calc: PowerAmountCalc,
    ) -> PowerAmountCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.modify_power_amount, calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.modify_power_amount, calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.modify_power_amount, calc),
            Self::Orb(id, rules) => apply_calc(ctx, id, rules.modify_power_amount, calc),
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.modify_power_amount, calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.modify_power_amount, calc),
        }
    }

    fn dispatch_modify_orb_passive_trigger_count(
        self,
        ctx: &RuleCtx<'_>,
        calc: OrbPassiveTriggerCountCalc,
    ) -> OrbPassiveTriggerCountCalc {
        match self {
            Self::Power(id, rules) => {
                apply_calc(ctx, id, rules.modify_orb_passive_trigger_count, calc)
            }
            Self::Relic(id, rules) => {
                apply_calc(ctx, id, rules.modify_orb_passive_trigger_count, calc)
            }
            Self::Potion(id, rules) => {
                apply_calc(ctx, id, rules.modify_orb_passive_trigger_count, calc)
            }
            Self::Orb(id, rules) => {
                apply_calc(ctx, id, rules.modify_orb_passive_trigger_count, calc)
            }
            Self::Monster(id, rules) => {
                apply_calc(ctx, id, rules.modify_orb_passive_trigger_count, calc)
            }
            Self::Card(id, rules) => {
                apply_calc(ctx, id, rules.modify_orb_passive_trigger_count, calc)
            }
        }
    }

    fn dispatch_modify_orb_value(self, ctx: &RuleCtx<'_>, calc: OrbValueCalc) -> OrbValueCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.modify_orb_value, calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.modify_orb_value, calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.modify_orb_value, calc),
            Self::Orb(id, rules) => apply_calc(ctx, id, rules.modify_orb_value, calc),
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.modify_orb_value, calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.modify_orb_value, calc),
        }
    }

    fn dispatch_modify_summon_amount(
        self,
        ctx: &RuleCtx<'_>,
        calc: SummonAmountCalc,
    ) -> SummonAmountCalc {
        match self {
            Self::Power(id, rules) => apply_calc(ctx, id, rules.modify_summon_amount, calc),
            Self::Relic(id, rules) => apply_calc(ctx, id, rules.modify_summon_amount, calc),
            Self::Potion(id, rules) => apply_calc(ctx, id, rules.modify_summon_amount, calc),
            Self::Orb(id, rules) => apply_calc(ctx, id, rules.modify_summon_amount, calc),
            Self::Monster(id, rules) => apply_calc(ctx, id, rules.modify_summon_amount, calc),
            Self::Card(id, rules) => apply_calc(ctx, id, rules.modify_summon_amount, calc),
        }
    }

    fn dispatch_decision(self, ctx: &RuleCtx<'_>, query: &DecisionQuery) -> Decision {
        match self {
            Self::Power(id, rules) => apply_decision(ctx, id, rules.decide, query),
            Self::Relic(id, rules) => apply_decision(ctx, id, rules.decide, query),
            Self::Potion(id, rules) => apply_decision(ctx, id, rules.decide, query),
            Self::Orb(_, _) => Decision::Allow,
            Self::Monster(id, rules) => apply_decision(ctx, id, rules.decide, query),
            Self::Card(id, rules) => apply_decision(ctx, id, rules.decide, query),
        }
    }
}

trait DamageRuleSet<Id> {
    fn damage_rule(
        &self,
        phase: ModifierPhase,
    ) -> Option<for<'ctx> fn(&RuleCtx<'ctx>, Id, DamageCalc) -> DamageCalc>;
}

macro_rules! impl_damage_rule_set {
    ($rules:ty, $id:ty) => {
        impl DamageRuleSet<$id> for $rules {
            fn damage_rule(
                &self,
                phase: ModifierPhase,
            ) -> Option<for<'ctx> fn(&RuleCtx<'ctx>, $id, DamageCalc) -> DamageCalc> {
                match phase {
                    ModifierPhase::Additive => self.modify_damage_additive,
                    ModifierPhase::Multiplicative => self.modify_damage_multiplicative,
                    ModifierPhase::Capping => self.modify_damage_cap,
                    ModifierPhase::Replacement => None,
                }
            }
        }
    };
}

impl_damage_rule_set!(PowerRules, PowerInstanceId);
impl_damage_rule_set!(RelicRules, RelicInstanceId);
impl_damage_rule_set!(PotionRules, PotionInstanceId);
impl_damage_rule_set!(MonsterRules, CreatureId);
impl_damage_rule_set!(CardRules, CardInstanceId);

trait BlockRuleSet<Id> {
    fn block_rule(
        &self,
        phase: ModifierPhase,
    ) -> Option<for<'ctx> fn(&RuleCtx<'ctx>, Id, BlockCalc) -> BlockCalc>;
}

macro_rules! impl_block_rule_set {
    ($rules:ty, $id:ty) => {
        impl BlockRuleSet<$id> for $rules {
            fn block_rule(
                &self,
                phase: ModifierPhase,
            ) -> Option<for<'ctx> fn(&RuleCtx<'ctx>, $id, BlockCalc) -> BlockCalc> {
                match phase {
                    ModifierPhase::Additive => self.modify_block_additive,
                    ModifierPhase::Multiplicative => self.modify_block_multiplicative,
                    ModifierPhase::Capping | ModifierPhase::Replacement => None,
                }
            }
        }
    };
}

impl_block_rule_set!(PowerRules, PowerInstanceId);
impl_block_rule_set!(RelicRules, RelicInstanceId);
impl_block_rule_set!(PotionRules, PotionInstanceId);
impl_block_rule_set!(MonsterRules, CreatureId);
impl_block_rule_set!(CardRules, CardInstanceId);

fn apply_event<Id>(
    ctx: &RuleCtx<'_>,
    id: Id,
    rule: Option<for<'rule> fn(&RuleCtx<'rule>, Id, &Event) -> Vec<Effect>>,
    event: &Event,
) -> Vec<Effect>
where
    Id: Copy,
{
    rule.map(|rule| rule(ctx, id, event)).unwrap_or_default()
}

fn apply_calc<Id, Calc>(
    ctx: &RuleCtx<'_>,
    id: Id,
    rule: Option<for<'rule> fn(&RuleCtx<'rule>, Id, Calc) -> Calc>,
    calc: Calc,
) -> Calc
where
    Id: Copy,
    Calc: Clone,
{
    rule.map(|rule| rule(ctx, id, calc.clone())).unwrap_or(calc)
}

fn apply_decision<Id>(
    ctx: &RuleCtx<'_>,
    id: Id,
    rule: Option<for<'rule> fn(&RuleCtx<'rule>, Id, &DecisionQuery) -> Decision>,
    query: &DecisionQuery,
) -> Decision
where
    Id: Copy,
{
    rule.map(|rule| rule(ctx, id, query))
        .unwrap_or(Decision::Allow)
}

fn card_def<'a>(
    registry: &'a StaticRegistry,
    state: &GameState,
    card: CardInstanceId,
) -> Option<&'a crate::content::cards::CardDef> {
    state
        .card(card)
        .and_then(|card| registry.cards.get(card.def))
}

fn event_listener_scope(event: &Event) -> ListenerScope {
    // Post-fact events stay combat broadcasts; Query and Decision points carry narrower scopes.
    match event {
        Event::CombatStarted
        | Event::TurnStarted { .. }
        | Event::TurnEnded { .. }
        | Event::BeforeHandDraw { .. }
        | Event::CardsShuffled(_)
        | Event::CardDrawn(_)
        | Event::CardDiscarded(_)
        | Event::CardExhausted(_)
        | Event::CardUpgraded(_)
        | Event::CardPlayStarted(_)
        | Event::CardPlayed(_)
        | Event::DamageDealt(_)
        | Event::BlockGained(_)
        | Event::PowerApplied(_)
        | Event::PowerAmountChanged(_)
        | Event::ResourceSpent(_)
        | Event::ResourceGained(_)
        | Event::OrbChanneled(_)
        | Event::OrbEvoked(_)
        | Event::Summoned(_)
        | Event::CreatureHpChanged(_)
        | Event::DeathPrevented { .. }
        | Event::CreatureDied { .. } => ListenerScope::Combat,
    }
}

fn damage_listener_scope(calc: &DamageCalc) -> ListenerScope {
    ListenerScope::related(
        related_creatures([calc.dealer, Some(calc.target)]),
        calc.source,
    )
}

fn block_listener_scope(calc: &BlockCalc) -> ListenerScope {
    ListenerScope::related([calc.target], calc.source)
}

fn hp_loss_listener_scope(calc: &HpLossCalc) -> ListenerScope {
    ListenerScope::related(
        related_creatures([calc.dealer, Some(calc.target)]),
        calc.source,
    )
}

fn unblocked_target_listener_scope(calc: &UnblockedDamageTargetCalc) -> ListenerScope {
    ListenerScope::related(
        related_creatures([calc.dealer, Some(calc.original_target), Some(calc.target)]),
        calc.source,
    )
}

fn resource_cost_listener_scope(state: &GameState, calc: &ResourceCostCalc) -> ListenerScope {
    ListenerScope::related(
        player_creature_for(state, calc.player),
        Some(Source::Card(calc.card)),
    )
}

fn card_result_pile_listener_scope(
    state: &GameState,
    calc: &CardPlayResultPileCalc,
) -> ListenerScope {
    ListenerScope::related(
        card_owner_creature(state, calc.card),
        Some(Source::Card(calc.card)),
    )
}

fn power_amount_listener_scope(calc: &PowerAmountCalc) -> ListenerScope {
    ListenerScope::related(
        related_creatures([calc.giver, Some(calc.target)]),
        calc.source,
    )
}

fn player_listener_scope(state: &GameState, player: PlayerId) -> ListenerScope {
    ListenerScope::related(player_creature_for(state, player), None)
}

fn decision_listener_scope(state: &GameState, query: &DecisionQuery) -> ListenerScope {
    match query.kind {
        DecisionQueryKind::ShouldPlay { card, target } => {
            let source = query.source.or(Some(Source::Card(card)));
            ListenerScope::related(
                related_creatures([card_owner_creature(state, card), target]),
                source,
            )
        }
        DecisionQueryKind::ShouldDraw { player, .. }
        | DecisionQueryKind::ShouldFlush { player }
        | DecisionQueryKind::ShouldTakeExtraTurn { player } => {
            ListenerScope::related(player_creature_for(state, player), query.source)
        }
        DecisionQueryKind::ShouldDie { creature }
        | DecisionQueryKind::ShouldRemoveCreatureAfterDeath { creature }
        | DecisionQueryKind::ShouldClearBlock { creature } => {
            ListenerScope::related([creature], query.source)
        }
        DecisionQueryKind::ShouldStopCombatFromEnding
        | DecisionQueryKind::ShouldStartTurn { .. } => ListenerScope::Combat,
    }
}

fn related_creatures<const N: usize>(creatures: [Option<CreatureId>; N]) -> Vec<CreatureId> {
    let mut out = Vec::new();
    for creature in creatures.into_iter().flatten() {
        if !out.contains(&creature) {
            out.push(creature);
        }
    }
    out
}

fn player_creature_for(state: &GameState, player: PlayerId) -> Option<CreatureId> {
    state.combat().and_then(|combat| {
        if combat.player.id == player {
            Some(combat.player.creature)
        } else {
            None
        }
    })
}

fn card_owner_creature(state: &GameState, card: CardInstanceId) -> Option<CreatureId> {
    let owner = state.card(card)?.owner;
    player_creature_for(state, owner)
}

pub fn prevent_by_current_listener(ctx: &RuleCtx<'_>, reason: PreventReason) -> Decision {
    match ctx.listener {
        Some(by) => Decision::Prevent { by, reason },
        None => Decision::Allow,
    }
}
