use std::collections::BTreeMap;

use crate::content::cards::{DEFEND_IRONCLAD, STRIKE_IRONCLAD};
use crate::content::monsters::NIBBIT;
use crate::core::ids::{CardId, CardInstanceId, CombatId, CreatureId, PlayerId};
use crate::core::rng::RngSet;
use crate::core::state::{
    CardCosts, CardFlags, CardInstance, CardPiles, CombatPhase, CombatSetupCard,
    CombatSetupMonster, CombatState, CombatStats, CombatTurnStats, Creature, GameState, PileId,
    PileKind, PlayerState, RunState, Side, TemporaryCardCosts, MAX_CARDS_IN_HAND,
};

impl GameState {
    pub fn single_player_test_combat(
        seed: u64,
        deck: impl IntoIterator<Item = CombatSetupCard>,
        monsters: impl IntoIterator<Item = CombatSetupMonster>,
        player_max_hp: i32,
        player_max_energy: i32,
        initial_draw_count: u8,
    ) -> Self {
        let player_id = PlayerId::new(1);
        let player_creature = CreatureId::new(1);
        let draw = PileId::player(player_id, PileKind::Draw);
        let hand = PileId::player(player_id, PileKind::Hand);

        let mut cards = BTreeMap::new();
        let mut draw_cards = Vec::new();
        let mut next_card_instance = 1;
        for setup in deck {
            let id = CardInstanceId::new(next_card_instance);
            next_card_instance += 1;
            cards.insert(
                id,
                CardInstance {
                    id,
                    def: setup.def,
                    owner: player_id,
                    upgraded: setup.upgraded,
                    costs: setup.costs,
                    temp_costs: TemporaryCardCosts::default(),
                    pile: draw,
                    flags: CardFlags::default(),
                    counters: BTreeMap::new(),
                },
            );
            draw_cards.push(id);
        }

        let mut rng = RngSet::seeded(seed);
        rng.shuffle.shuffle(&mut draw_cards);

        let mut hand_cards = Vec::new();
        for _ in 0..usize::from(initial_draw_count).min(MAX_CARDS_IN_HAND) {
            let Some(card) = draw_cards.pop() else {
                break;
            };
            if let Some(card_state) = cards.get_mut(&card) {
                card_state.pile = hand;
            }
            hand_cards.push(card);
        }

        let mut creatures = vec![Creature::new(player_creature, Side::Player, player_max_hp)];
        let mut next_creature = 2;
        for monster in monsters {
            let id = CreatureId::new(next_creature);
            next_creature += 1;
            let mut creature = Creature::new(id, Side::Monsters, monster.max_hp);
            if let Some(model) = monster.model {
                creature = creature.with_model(model);
            }
            creatures.push(creature);
        }

        let mut piles = CardPiles::default();
        piles.draw = draw_cards;
        piles.hand = hand_cards;

        let combat = CombatState {
            id: CombatId::new(1),
            phase: CombatPhase::PlayerAction,
            player: PlayerState {
                id: player_id,
                creature: player_creature,
                energy: player_max_energy,
                max_energy: player_max_energy,
                stars: 0,
                relics: Vec::new(),
                potions: Vec::new(),
                piles,
            },
            creatures,
            cards,
            powers: BTreeMap::new(),
            relics: BTreeMap::new(),
            potions: BTreeMap::new(),
            modifiers: Vec::new(),
            turn_stats: CombatTurnStats::default(),
            combat_stats: CombatStats::default(),
            next_card_instance,
            next_power_instance: 1,
        };

        Self {
            run: RunState { seed },
            combat: Some(combat),
            rng,
        }
    }

    pub fn demo_combat(seed: u64) -> Self {
        let player_id = PlayerId::new(1);
        let player_creature = CreatureId::new(1);
        let enemy = CreatureId::new(2);
        let strike = CardInstanceId::new(1);
        let hand = PileId::player(player_id, PileKind::Hand);

        let mut cards = BTreeMap::new();
        cards.insert(
            strike,
            CardInstance {
                id: strike,
                def: CardId::new("starter_strike"),
                owner: player_id,
                upgraded: false,
                costs: CardCosts::energy(1),
                temp_costs: TemporaryCardCosts::default(),
                pile: hand,
                flags: CardFlags::default(),
                counters: BTreeMap::new(),
            },
        );

        let mut piles = CardPiles::default();
        piles.hand.push(strike);

        let combat = CombatState {
            id: CombatId::new(1),
            phase: CombatPhase::PlayerAction,
            player: PlayerState {
                id: player_id,
                creature: player_creature,
                energy: 3,
                max_energy: 3,
                stars: 0,
                relics: Vec::new(),
                potions: Vec::new(),
                piles,
            },
            creatures: vec![
                Creature::new(player_creature, Side::Player, 50),
                Creature::new(enemy, Side::Monsters, 30),
            ],
            cards,
            powers: BTreeMap::new(),
            relics: BTreeMap::new(),
            potions: BTreeMap::new(),
            modifiers: Vec::new(),
            turn_stats: CombatTurnStats::default(),
            combat_stats: CombatStats::default(),
            next_card_instance: 2,
            next_power_instance: 1,
        };

        Self::with_combat(seed, combat)
    }

    pub fn basic_nibbit_combat(seed: u64) -> Self {
        let player_id = PlayerId::new(1);
        let player_creature = CreatureId::new(1);
        let enemy = CreatureId::new(2);
        let strike = CardInstanceId::new(1);
        let defend = CardInstanceId::new(2);
        let hand = PileId::player(player_id, PileKind::Hand);

        let mut cards = BTreeMap::new();
        cards.insert(
            strike,
            CardInstance {
                id: strike,
                def: STRIKE_IRONCLAD,
                owner: player_id,
                upgraded: false,
                costs: CardCosts::energy(1),
                temp_costs: TemporaryCardCosts::default(),
                pile: hand,
                flags: CardFlags::default(),
                counters: BTreeMap::new(),
            },
        );
        cards.insert(
            defend,
            CardInstance {
                id: defend,
                def: DEFEND_IRONCLAD,
                owner: player_id,
                upgraded: false,
                costs: CardCosts::energy(1),
                temp_costs: TemporaryCardCosts::default(),
                pile: hand,
                flags: CardFlags::default(),
                counters: BTreeMap::new(),
            },
        );

        let mut piles = CardPiles::default();
        piles.hand.extend([strike, defend]);

        let combat = CombatState {
            id: CombatId::new(1),
            phase: CombatPhase::PlayerAction,
            player: PlayerState {
                id: player_id,
                creature: player_creature,
                energy: 3,
                max_energy: 3,
                stars: 0,
                relics: Vec::new(),
                potions: Vec::new(),
                piles,
            },
            creatures: vec![
                Creature::new(player_creature, Side::Player, 50),
                Creature::new(enemy, Side::Monsters, 42).with_model(NIBBIT),
            ],
            cards,
            powers: BTreeMap::new(),
            relics: BTreeMap::new(),
            potions: BTreeMap::new(),
            modifiers: Vec::new(),
            turn_stats: CombatTurnStats::default(),
            combat_stats: CombatStats::default(),
            next_card_instance: 3,
            next_power_instance: 1,
        };

        Self::with_combat(seed, combat)
    }

    pub fn full_nibbit_combat(seed: u64) -> Self {
        let player_id = PlayerId::new(1);
        let player_creature = CreatureId::new(1);
        let enemy = CreatureId::new(2);
        let draw = PileId::player(player_id, PileKind::Draw);
        let hand = PileId::player(player_id, PileKind::Hand);

        let mut cards = BTreeMap::new();
        let mut draw_cards = Vec::new();
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

        for (index, def) in starter_defs.into_iter().enumerate() {
            let id = CardInstanceId::new((index + 1) as u32);
            cards.insert(
                id,
                CardInstance {
                    id,
                    def,
                    owner: player_id,
                    upgraded: false,
                    costs: CardCosts::energy(1),
                    temp_costs: TemporaryCardCosts::default(),
                    pile: draw,
                    flags: CardFlags::default(),
                    counters: BTreeMap::new(),
                },
            );
            draw_cards.push(id);
        }

        let mut rng = RngSet::seeded(seed);
        rng.shuffle.shuffle(&mut draw_cards);

        let mut hand_cards = Vec::new();
        for _ in 0..2 {
            if let Some(card) = draw_cards.pop() {
                if let Some(card_state) = cards.get_mut(&card) {
                    card_state.pile = hand;
                }
                hand_cards.push(card);
            }
        }

        let mut piles = CardPiles::default();
        piles.draw = draw_cards;
        piles.hand = hand_cards;

        let combat = CombatState {
            id: CombatId::new(1),
            phase: CombatPhase::PlayerAction,
            player: PlayerState {
                id: player_id,
                creature: player_creature,
                energy: 3,
                max_energy: 3,
                stars: 0,
                relics: Vec::new(),
                potions: Vec::new(),
                piles,
            },
            creatures: vec![
                Creature::new(player_creature, Side::Player, 50),
                Creature::new(enemy, Side::Monsters, 42).with_model(NIBBIT),
            ],
            cards,
            powers: BTreeMap::new(),
            relics: BTreeMap::new(),
            potions: BTreeMap::new(),
            modifiers: Vec::new(),
            turn_stats: CombatTurnStats::default(),
            combat_stats: CombatStats::default(),
            next_card_instance: 10,
            next_power_instance: 1,
        };

        Self {
            run: RunState { seed },
            combat: Some(combat),
            rng,
        }
    }
}
