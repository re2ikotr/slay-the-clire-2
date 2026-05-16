use crate::content::cards::{DEFEND_IRONCLAD, STRIKE_IRONCLAD};
use crate::content::monsters::MonsterIntent;
use crate::core::ids::{CardInstanceId, CreatureId};
use crate::core::rules::RuleCtx;
use crate::core::{CombatPhase, Command, Engine, GameState, Side, StepResult};

pub fn run() {
    let mut engine = Engine::new(GameState::full_nibbit_combat(0));
    let player = engine.state.player_id().expect("demo combat has a player");
    let enemy = engine
        .state
        .combat()
        .and_then(|combat| combat.monster_ids().first().copied())
        .expect("demo combat has a monster");

    println!("slay-the-clire-2 full combat smoke");
    print_state("start", &engine, enemy);

    for turn in 1..=20 {
        if engine
            .state
            .combat()
            .map(|combat| combat.phase != CombatPhase::PlayerAction)
            .unwrap_or(true)
        {
            break;
        }

        println!("turn {turn}");
        while let Some((card, target, label)) = next_card_to_play(&engine, enemy) {
            let finished = print_result(
                &format!("play {label} {:?}", card),
                engine.step(Command::PlayCard {
                    player,
                    card,
                    target,
                }),
            );
            print_state("after play", &engine, enemy);
            if finished {
                return;
            }
        }

        let finished = print_result(
            "end turn",
            engine.step(Command::EndTurn { side: Side::Player }),
        );
        print_state("after turn cycle", &engine, enemy);
        if finished {
            return;
        }
    }
}

fn next_card_to_play(
    engine: &Engine,
    enemy: CreatureId,
) -> Option<(CardInstanceId, Option<CreatureId>, &'static str)> {
    let combat = engine.state.combat()?;
    if combat.player.energy <= 0 {
        return None;
    }

    let enemy_alive = engine
        .state
        .creature(enemy)
        .map(|creature| creature.alive)
        .unwrap_or(false);

    let mut fallback = None;
    for card in &combat.player.piles.hand {
        let card_state = combat.cards.get(card)?;
        let costs = card_state.effective_costs();
        let Some(energy) = costs.energy.amount_to_pay(combat.player.energy) else {
            continue;
        };
        let Some(stars) = costs.stars.amount_to_pay(combat.player.stars) else {
            continue;
        };
        if energy > combat.player.energy || stars > combat.player.stars {
            continue;
        }

        if card_state.def == STRIKE_IRONCLAD && enemy_alive {
            return Some((*card, Some(enemy), "Strike"));
        }
        if card_state.def == DEFEND_IRONCLAD {
            fallback.get_or_insert((*card, None, "Defend"));
        }
    }

    fallback
}

fn print_result(label: &str, result: StepResult) -> bool {
    match result {
        StepResult::Done(log) => {
            println!("{label}: done ({} log entries)", log.len());
            false
        }
        StepResult::NeedChoice(_, log) => {
            println!("{label}: choice requested after {} log entries", log.len());
            false
        }
        StepResult::CombatOver(result, log) => {
            println!(
                "{label}: combat ended as {:?} after {} log entries",
                result.outcome,
                log.len()
            );
            true
        }
        StepResult::Rejected(error, log) => {
            println!("{label}: rejected after {} log entries: {error}", log.len());
            false
        }
        StepResult::Failed(error, log) => {
            println!("{label}: failed after {} log entries: {error}", log.len());
            false
        }
    }
}

fn print_state(label: &str, engine: &Engine, enemy: CreatureId) {
    let combat = engine.state.combat().expect("combat is active");
    let player = engine
        .state
        .creature(combat.player.creature)
        .expect("player creature exists");
    let monster = engine.state.creature(enemy).expect("monster exists");
    let intent = monster_intent(engine, enemy);

    println!(
        "{label}: player hp={} block={} energy={} hand={} draw={} discard={} | nibbit hp={} block={} intent={intent:?}",
        player.hp,
        player.block,
        combat.player.energy,
        combat.player.piles.hand.len(),
        combat.player.piles.draw.len(),
        combat.player.piles.discard.len(),
        monster.hp,
        monster.block
    );
}

fn monster_intent(engine: &Engine, enemy: CreatureId) -> MonsterIntent {
    let Some(monster) = engine.state.creature(enemy) else {
        return MonsterIntent::Unknown;
    };
    let Some(model) = monster.model else {
        return MonsterIntent::Unknown;
    };
    let Some(def) = engine.registry.monsters.get(model) else {
        return MonsterIntent::Unknown;
    };
    let ctx = RuleCtx {
        state: &engine.state,
        listener: None,
    };
    (def.intent)(&ctx, enemy)
}
