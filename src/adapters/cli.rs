use crate::core::{Command, Engine, GameState, Side, StepResult};

pub fn run() {
    let mut engine = Engine::new(GameState::demo_combat(0));
    let result = engine.step(Command::EndTurn { side: Side::Player });

    println!("slay-the-clire-2 logic skeleton");
    match result {
        StepResult::Done(log) => {
            println!("resolved end turn with {} log entries", log.len());
        }
        StepResult::NeedChoice(_, log) => {
            println!("choice requested after {} log entries", log.len());
        }
        StepResult::CombatOver(result, log) => {
            println!(
                "combat ended as {:?} after {} log entries",
                result.outcome,
                log.len()
            );
        }
        StepResult::Rejected(error, log) => {
            println!("command rejected after {} log entries: {error}", log.len());
        }
        StepResult::Failed(error, log) => {
            println!("resolver failed after {} log entries: {error}", log.len());
        }
    }
}
