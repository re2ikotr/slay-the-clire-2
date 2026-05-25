use crate::content::cards::{DEFEND_IRONCLAD, STRIKE_IRONCLAD};
use crate::content::monsters::{nibbit, NIBBIT};
use crate::core::build_single_player_combat;
use crate::core::rng::RngSet;
use crate::core::state::{CardCosts, CombatSetupCard, CombatSetupMonster, GameState};
use crate::registry::StaticRegistry;

pub fn full_nibbit_combat(seed: u64) -> GameState {
    let starter_defs = [
        STRIKE_IRONCLAD,
        STRIKE_IRONCLAD,
        STRIKE_IRONCLAD,
        STRIKE_IRONCLAD,
        STRIKE_IRONCLAD,
        DEFEND_IRONCLAD,
        DEFEND_IRONCLAD,
        DEFEND_IRONCLAD,
        DEFEND_IRONCLAD,
    ];
    let deck = starter_defs.into_iter().map(|def| CombatSetupCard {
        def,
        upgraded: false,
        costs: CardCosts::energy(1),
    });
    let nibbit = nibbit();

    build_single_player_combat(
        seed,
        deck,
        [CombatSetupMonster {
            model: Some(nibbit.id),
            max_hp: nibbit.max_hp,
        }],
        50,
        3,
        2,
    )
}

pub fn random_nibbit_combat(
    registry: &StaticRegistry,
    seed: u64,
    nibbit_count: usize,
    deck_size: usize,
    player_max_hp: i32,
    player_max_energy: i32,
    initial_draw_count: u8,
) -> GameState {
    let deck = random_combat_deck(registry, seed, deck_size);
    let fallback = nibbit();
    let monster = registry
        .monsters
        .get(NIBBIT)
        .map(|def| CombatSetupMonster {
            model: Some(def.id),
            max_hp: def.max_hp,
        })
        .unwrap_or(CombatSetupMonster {
            model: None,
            max_hp: fallback.max_hp,
        });
    let monsters = std::iter::repeat(monster).take(nibbit_count.max(1));

    build_single_player_combat(
        seed,
        deck,
        monsters,
        player_max_hp,
        player_max_energy,
        initial_draw_count,
    )
}

fn random_combat_deck(registry: &StaticRegistry, seed: u64, count: usize) -> Vec<CombatSetupCard> {
    let candidates = registry
        .cards
        .values()
        .filter(|def| def.can_generate_in_combat)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut rng = RngSet::seeded(seed);
    let mut deck = Vec::with_capacity(count);
    for _ in 0..count {
        let Some(index) = rng.combat_card_generation.next_usize(candidates.len()) else {
            break;
        };
        let def = candidates[index];
        deck.push(CombatSetupCard {
            def: def.id,
            upgraded: false,
            costs: def.costs_for(false),
        });
    }
    deck
}
