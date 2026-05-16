use crate::content::cards::{DEFEND_IRONCLAD, STRIKE_IRONCLAD};
use crate::content::monsters::MonsterIntent;
use crate::core::ids::{CardInstanceId, CreatureId};
use crate::core::rules::RuleCtx;
use crate::core::{Command, Engine, GameState, Side, StepResult};

pub fn run() {
    let mut engine = Engine::new(GameState::basic_nibbit_combat(0));
    let player = engine.state.player_id().expect("demo combat has a player");
    let enemy = engine
        .state
        .combat()
        .and_then(|combat| combat.monster_ids().first().copied())
        .expect("demo combat has a monster");
    let (strike, defend) = starter_cards(&engine);

    println!("slay-the-clire-2 basic combat smoke");
    print_state("start", &engine, enemy);
    print_result(
        "play Defend",
        engine.step(Command::PlayCard {
            player,
            card: defend,
            target: None,
        }),
    );
    print_state("after Defend", &engine, enemy);
    print_result(
        "play Strike",
        engine.step(Command::PlayCard {
            player,
            card: strike,
            target: Some(enemy),
        }),
    );
    print_state("after Strike", &engine, enemy);
    print_result(
        "end turn",
        engine.step(Command::EndTurn { side: Side::Player }),
    );
    print_state("next player turn", &engine, enemy);
}

fn starter_cards(engine: &Engine) -> (CardInstanceId, CardInstanceId) {
    let combat = engine.state.combat().expect("demo combat is active");
    let mut strike = None;
    let mut defend = None;
    for card in &combat.player.piles.hand {
        let card_state = combat.cards.get(card).expect("hand card exists");
        if card_state.def == STRIKE_IRONCLAD {
            strike = Some(*card);
        } else if card_state.def == DEFEND_IRONCLAD {
            defend = Some(*card);
        }
    }
    (
        strike.expect("demo hand has Strike"),
        defend.expect("demo hand has Defend"),
    )
}

fn print_result(label: &str, result: StepResult) {
    match result {
        StepResult::Done(log) => {
            println!("{label}: done ({} log entries)", log.len());
        }
        StepResult::NeedChoice(_, log) => {
            println!("{label}: choice requested after {} log entries", log.len());
        }
        StepResult::CombatOver(result, log) => {
            println!(
                "{label}: combat ended as {:?} after {} log entries",
                result.outcome,
                log.len()
            );
        }
        StepResult::Rejected(error, log) => {
            println!("{label}: rejected after {} log entries: {error}", log.len());
        }
        StepResult::Failed(error, log) => {
            println!("{label}: failed after {} log entries: {error}", log.len());
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
        "{label}: player hp={} block={} energy={} hand={} | nibbit hp={} block={} intent={intent:?}",
        player.hp,
        player.block,
        combat.player.energy,
        combat.player.piles.hand.len(),
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
