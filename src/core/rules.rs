use rust_decimal::Decimal;

use crate::core::effect::{Effect, Source};
use crate::core::event::Event;
use crate::core::ids::{CardInstanceId, CreatureId};
use crate::core::listener::{collect_combat_listeners, ListenerRef, ListenerScope};
use crate::core::query::{
    BlockCalc, DamageCalc, Decision, DecisionQuery, DecisionQueryKind, ModifierLog, ModifierPhase,
    PreventReason, ResourceCostCalc,
};
use crate::core::state::GameState;
use crate::registry::StaticRegistry;

#[derive(Default)]
pub struct RulePipeline;

impl RulePipeline {
    pub fn notify(registry: &StaticRegistry, state: &GameState, event: &Event) -> Vec<Effect> {
        let listeners = collect_combat_listeners(state, event_scope(event));
        let mut out = Vec::new();

        for listener in listeners {
            let ctx = RuleCtx {
                state,
                registry,
                listener: Some(listener),
            };
            out.extend(dispatch_event(registry, state, listener, &ctx, event));
        }

        out
    }

    pub fn modify_damage(
        registry: &StaticRegistry,
        state: &GameState,
        calc: DamageCalc,
    ) -> (DamageCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(
            state,
            calc.source
                .map(ListenerScope::Source)
                .unwrap_or(ListenerScope::Creature(calc.target)),
        );
        let mut calc = calc;
        let mut logs = Vec::new();

        for phase in [
            ModifierPhase::Additive,
            ModifierPhase::Multiplicative,
            ModifierPhase::Capping,
        ] {
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
        let listeners = collect_combat_listeners(
            state,
            calc.source
                .map(ListenerScope::Source)
                .unwrap_or(ListenerScope::Creature(calc.target)),
        );
        let mut calc = calc;
        let mut logs = Vec::new();

        for phase in [ModifierPhase::Additive, ModifierPhase::Multiplicative] {
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

    pub fn modify_resource_cost(
        registry: &StaticRegistry,
        state: &GameState,
        calc: ResourceCostCalc,
    ) -> (ResourceCostCalc, Vec<ModifierLog>) {
        let listeners = collect_combat_listeners(state, ListenerScope::Combat);
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

    pub fn decide(
        registry: &StaticRegistry,
        state: &GameState,
        scope: ListenerScope,
        query: DecisionQuery,
    ) -> Decision {
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
            ListenerScope::Combat,
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
            ListenerScope::Combat,
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
            ListenerScope::Creature(creature),
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
            ListenerScope::Creature(creature),
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
            ListenerScope::Combat,
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
    match listener {
        ListenerRef::Power(id) => state
            .combat()
            .and_then(|combat| combat.powers.get(&id))
            .and_then(|instance| registry.powers.get(instance.def))
            .and_then(|def| def.rules.on_event)
            .map(|rule| rule(ctx, id, event))
            .unwrap_or_default(),
        ListenerRef::Relic(id) => state
            .combat()
            .and_then(|combat| combat.relics.get(&id))
            .and_then(|instance| registry.relics.get(instance.def))
            .and_then(|def| def.rules.on_event)
            .map(|rule| rule(ctx, id, event))
            .unwrap_or_default(),
        ListenerRef::Potion(id) => state
            .combat()
            .and_then(|combat| combat.potions.get(&id))
            .and_then(|instance| registry.potions.get(instance.def))
            .and_then(|def| def.rules.on_event)
            .map(|rule| rule(ctx, id, event))
            .unwrap_or_default(),
        ListenerRef::Monster(id) => state
            .creature(id)
            .and_then(|creature| creature.model)
            .and_then(|model| registry.monsters.get(model))
            .and_then(|def| def.rules.on_event)
            .map(|rule| rule(ctx, id, event))
            .unwrap_or_default(),
        ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
            card_def(registry, state, id)
                .and_then(|def| def.rules.on_event)
                .map(|rule| rule(ctx, id, event))
                .unwrap_or_default()
        }
        ListenerRef::Orb(_) | ListenerRef::Modifier(_) => Vec::new(),
    }
}

fn dispatch_modify_damage(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    phase: ModifierPhase,
    calc: DamageCalc,
) -> DamageCalc {
    match listener {
        ListenerRef::Power(id) => state
            .combat()
            .and_then(|combat| combat.powers.get(&id))
            .and_then(|instance| registry.powers.get(instance.def))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_damage_additive,
                ModifierPhase::Multiplicative => def.rules.modify_damage_multiplicative,
                ModifierPhase::Capping => def.rules.modify_damage_cap,
                ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Relic(id) => state
            .combat()
            .and_then(|combat| combat.relics.get(&id))
            .and_then(|instance| registry.relics.get(instance.def))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_damage_additive,
                ModifierPhase::Multiplicative => def.rules.modify_damage_multiplicative,
                ModifierPhase::Capping => def.rules.modify_damage_cap,
                ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Potion(id) => state
            .combat()
            .and_then(|combat| combat.potions.get(&id))
            .and_then(|instance| registry.potions.get(instance.def))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_damage_additive,
                ModifierPhase::Multiplicative => def.rules.modify_damage_multiplicative,
                ModifierPhase::Capping => def.rules.modify_damage_cap,
                ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Monster(id) => state
            .creature(id)
            .and_then(|creature| creature.model)
            .and_then(|model| registry.monsters.get(model))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_damage_additive,
                ModifierPhase::Multiplicative => def.rules.modify_damage_multiplicative,
                ModifierPhase::Capping => def.rules.modify_damage_cap,
                ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
            card_def(registry, state, id)
                .and_then(|def| match phase {
                    ModifierPhase::Additive => def.rules.modify_damage_additive,
                    ModifierPhase::Multiplicative => def.rules.modify_damage_multiplicative,
                    ModifierPhase::Capping => def.rules.modify_damage_cap,
                    ModifierPhase::Replacement => None,
                })
                .map(|rule| rule(ctx, id, calc.clone()))
                .unwrap_or(calc)
        }
        ListenerRef::Orb(_) | ListenerRef::Modifier(_) => calc,
    }
}

fn dispatch_modify_block(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    phase: ModifierPhase,
    calc: BlockCalc,
) -> BlockCalc {
    match listener {
        ListenerRef::Power(id) => state
            .combat()
            .and_then(|combat| combat.powers.get(&id))
            .and_then(|instance| registry.powers.get(instance.def))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_block_additive,
                ModifierPhase::Multiplicative => def.rules.modify_block_multiplicative,
                ModifierPhase::Capping | ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Relic(id) => state
            .combat()
            .and_then(|combat| combat.relics.get(&id))
            .and_then(|instance| registry.relics.get(instance.def))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_block_additive,
                ModifierPhase::Multiplicative => def.rules.modify_block_multiplicative,
                ModifierPhase::Capping | ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Potion(id) => state
            .combat()
            .and_then(|combat| combat.potions.get(&id))
            .and_then(|instance| registry.potions.get(instance.def))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_block_additive,
                ModifierPhase::Multiplicative => def.rules.modify_block_multiplicative,
                ModifierPhase::Capping | ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Monster(id) => state
            .creature(id)
            .and_then(|creature| creature.model)
            .and_then(|model| registry.monsters.get(model))
            .and_then(|def| match phase {
                ModifierPhase::Additive => def.rules.modify_block_additive,
                ModifierPhase::Multiplicative => def.rules.modify_block_multiplicative,
                ModifierPhase::Capping | ModifierPhase::Replacement => None,
            })
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
            card_def(registry, state, id)
                .and_then(|def| match phase {
                    ModifierPhase::Additive => def.rules.modify_block_additive,
                    ModifierPhase::Multiplicative => def.rules.modify_block_multiplicative,
                    ModifierPhase::Capping | ModifierPhase::Replacement => None,
                })
                .map(|rule| rule(ctx, id, calc.clone()))
                .unwrap_or(calc)
        }
        ListenerRef::Orb(_) | ListenerRef::Modifier(_) => calc,
    }
}

fn dispatch_modify_resource_cost(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    calc: ResourceCostCalc,
) -> ResourceCostCalc {
    match listener {
        ListenerRef::Power(id) => state
            .combat()
            .and_then(|combat| combat.powers.get(&id))
            .and_then(|instance| registry.powers.get(instance.def))
            .and_then(|def| def.rules.modify_resource_cost)
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Relic(id) => state
            .combat()
            .and_then(|combat| combat.relics.get(&id))
            .and_then(|instance| registry.relics.get(instance.def))
            .and_then(|def| def.rules.modify_resource_cost)
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Potion(id) => state
            .combat()
            .and_then(|combat| combat.potions.get(&id))
            .and_then(|instance| registry.potions.get(instance.def))
            .and_then(|def| def.rules.modify_resource_cost)
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Monster(id) => state
            .creature(id)
            .and_then(|creature| creature.model)
            .and_then(|model| registry.monsters.get(model))
            .and_then(|def| def.rules.modify_resource_cost)
            .map(|rule| rule(ctx, id, calc.clone()))
            .unwrap_or(calc),
        ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
            card_def(registry, state, id)
                .and_then(|def| def.rules.modify_resource_cost)
                .map(|rule| rule(ctx, id, calc.clone()))
                .unwrap_or(calc)
        }
        ListenerRef::Orb(_) | ListenerRef::Modifier(_) => calc,
    }
}

fn dispatch_decision(
    registry: &StaticRegistry,
    state: &GameState,
    listener: ListenerRef,
    ctx: &RuleCtx<'_>,
    query: &DecisionQuery,
) -> Decision {
    match listener {
        ListenerRef::Power(id) => state
            .combat()
            .and_then(|combat| combat.powers.get(&id))
            .and_then(|instance| registry.powers.get(instance.def))
            .and_then(|def| def.rules.decide)
            .map(|rule| rule(ctx, id, query))
            .unwrap_or(Decision::Allow),
        ListenerRef::Relic(id) => state
            .combat()
            .and_then(|combat| combat.relics.get(&id))
            .and_then(|instance| registry.relics.get(instance.def))
            .and_then(|def| def.rules.decide)
            .map(|rule| rule(ctx, id, query))
            .unwrap_or(Decision::Allow),
        ListenerRef::Potion(id) => state
            .combat()
            .and_then(|combat| combat.potions.get(&id))
            .and_then(|instance| registry.potions.get(instance.def))
            .and_then(|def| def.rules.decide)
            .map(|rule| rule(ctx, id, query))
            .unwrap_or(Decision::Allow),
        ListenerRef::Monster(id) => state
            .creature(id)
            .and_then(|creature| creature.model)
            .and_then(|model| registry.monsters.get(model))
            .and_then(|def| def.rules.decide)
            .map(|rule| rule(ctx, id, query))
            .unwrap_or(Decision::Allow),
        ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
            card_def(registry, state, id)
                .and_then(|def| def.rules.decide)
                .map(|rule| rule(ctx, id, query))
                .unwrap_or(Decision::Allow)
        }
        ListenerRef::Orb(_) | ListenerRef::Modifier(_) => Decision::Allow,
    }
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

fn event_scope(event: &Event) -> ListenerScope {
    match event {
        Event::CardsShuffled(_) => ListenerScope::Combat,
        Event::CardDrawn(event) => ListenerScope::Source(Source::Card(event.card)),
        Event::CardExhausted(event) => ListenerScope::Source(Source::Card(event.card)),
        Event::CardUpgraded(event) => ListenerScope::Source(Source::Card(event.card)),
        Event::CardPlayStarted(event) => ListenerScope::Source(Source::Card(event.card)),
        Event::CardPlayed(event) => ListenerScope::Source(Source::Card(event.card)),
        Event::DamageDealt(event) => ListenerScope::Creature(event.target),
        Event::BlockGained(event) => ListenerScope::Creature(event.target),
        Event::PowerApplied(event) => ListenerScope::Creature(event.target),
        Event::ResourceSpent(_) | Event::ResourceGained(_) => ListenerScope::Combat,
        Event::CreatureHpChanged(event) => ListenerScope::Creature(event.creature),
        Event::DeathPrevented { creature } | Event::CreatureDied { creature } => {
            ListenerScope::Creature(*creature)
        }
        Event::CombatStarted | Event::TurnStarted { .. } | Event::TurnEnded { .. } => {
            ListenerScope::Combat
        }
    }
}

pub fn prevent_by_current_listener(ctx: &RuleCtx<'_>, reason: PreventReason) -> Decision {
    match ctx.listener {
        Some(by) => Decision::Prevent { by, reason },
        None => Decision::Allow,
    }
}
