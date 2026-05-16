use crate::core::effect::Source;
use crate::core::event::Event;
use crate::core::ids::{CardInstanceId, CreatureId};
use crate::core::listener::{collect_combat_listeners, ListenerScope};
use crate::core::query::{BlockCalc, DamageCalc, Decision, ModifierLog, ResourceCostCalc};
use crate::core::state::GameState;

#[derive(Default)]
pub struct RulePipeline;

impl RulePipeline {
    pub fn notify(state: &GameState, event: &Event) -> Vec<crate::core::effect::Effect> {
        let _listeners = collect_combat_listeners(state, event_scope(event));
        Vec::new()
    }

    pub fn modify_damage(state: &GameState, calc: DamageCalc) -> (DamageCalc, Vec<ModifierLog>) {
        let _listeners = collect_combat_listeners(
            state,
            calc.source
                .map(ListenerScope::Source)
                .unwrap_or(ListenerScope::Creature(calc.target)),
        );
        (calc, Vec::new())
    }

    pub fn modify_block(state: &GameState, calc: BlockCalc) -> (BlockCalc, Vec<ModifierLog>) {
        let _listeners = collect_combat_listeners(
            state,
            calc.source
                .map(ListenerScope::Source)
                .unwrap_or(ListenerScope::Creature(calc.target)),
        );
        (calc, Vec::new())
    }

    pub fn modify_resource_cost(
        state: &GameState,
        calc: ResourceCostCalc,
    ) -> (ResourceCostCalc, Vec<ModifierLog>) {
        let _listeners = collect_combat_listeners(state, ListenerScope::Combat);
        (calc, Vec::new())
    }

    pub fn should_play(
        state: &GameState,
        _card: CardInstanceId,
        _target: Option<CreatureId>,
    ) -> Decision {
        let _listeners = collect_combat_listeners(state, ListenerScope::Combat);
        Decision::Allow
    }

    pub fn should_die(state: &GameState, creature: CreatureId) -> Decision {
        let _listeners = collect_combat_listeners(state, ListenerScope::Creature(creature));
        Decision::Allow
    }
}

pub struct RuleCtx<'a> {
    pub state: &'a GameState,
    pub listener: Option<crate::core::listener::ListenerRef>,
}

fn event_scope(event: &Event) -> ListenerScope {
    match event {
        Event::CardDrawn(event) => ListenerScope::Source(Source::Card(event.card)),
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
