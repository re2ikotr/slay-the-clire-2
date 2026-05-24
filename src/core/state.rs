use std::collections::BTreeMap;
use std::fmt;

use rust_decimal::Decimal;

use crate::content::cards::{DEFEND_IRONCLAD, STRIKE_IRONCLAD};
use crate::content::monsters::NIBBIT;
use crate::core::ids::{
    CardId, CardInstanceId, CombatId, CreatureId, ModifierInstanceId, MonsterId, PlayerId,
    PotionId, PotionInstanceId, PowerId, PowerInstanceId, RelicId, RelicInstanceId,
};
use crate::core::rng::RngSet;

pub const MAX_CARDS_IN_HAND: usize = 10;
pub const BASE_HAND_DRAW_COUNT: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Player,
    Monsters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    Energy,
    Stars,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatPhase {
    CombatStart,
    PlayerStart,
    PlayerAction,
    PlayerEnd,
    EnemyAction,
    EnemyEnd,
    Victory,
    Defeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PileKind {
    Draw,
    Hand,
    Discard,
    Exhaust,
    Limbo,
    Play,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileId {
    pub owner: PlayerId,
    pub kind: PileKind,
}

impl PileId {
    pub const fn player(owner: PlayerId, kind: PileKind) -> Self {
        Self { owner, kind }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardCost {
    None,
    Fixed(i32),
    X,
    Unplayable,
}

impl CardCost {
    pub fn amount_to_pay(self, available: i32) -> Option<i32> {
        match self {
            Self::None => Some(0),
            Self::Fixed(value) if value >= 0 => Some(value),
            Self::X => Some(available.max(0)),
            Self::Fixed(_) | Self::Unplayable => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardCosts {
    pub energy: CardCost,
    pub stars: CardCost,
}

impl CardCosts {
    pub const ZERO: Self = Self {
        energy: CardCost::Fixed(0),
        stars: CardCost::None,
    };

    pub const fn energy(amount: i32) -> Self {
        Self {
            energy: CardCost::Fixed(amount),
            stars: CardCost::None,
        }
    }

    pub const fn x_energy() -> Self {
        Self {
            energy: CardCost::X,
            stars: CardCost::None,
        }
    }
}

impl Default for CardCosts {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TemporaryCardCosts {
    pub energy: Option<CardCost>,
    pub stars: Option<CardCost>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardFlags {
    pub ethereal: bool,
    pub temporary: bool,
    pub purge_on_use: bool,
    pub zero_cost_this_turn: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardCounter {
    DamageIncrease,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardInstance {
    pub id: CardInstanceId,
    pub def: CardId,
    pub owner: PlayerId,
    pub upgraded: bool,
    pub costs: CardCosts,
    pub temp_costs: TemporaryCardCosts,
    pub pile: PileId,
    pub flags: CardFlags,
    pub counters: BTreeMap<CardCounter, i32>,
}

impl CardInstance {
    pub fn effective_costs(&self) -> CardCosts {
        self.costs_with_temporary(self.costs)
    }

    pub fn costs_with_temporary(&self, base_costs: CardCosts) -> CardCosts {
        CardCosts {
            energy: self.temp_costs.energy.unwrap_or(base_costs.energy),
            stars: self.temp_costs.stars.unwrap_or(base_costs.stars),
        }
    }

    pub fn counter(&self, counter: CardCounter) -> i32 {
        self.counters.get(&counter).copied().unwrap_or(0)
    }

    fn clear_turn_limited_state(&mut self) {
        if self.flags.zero_cost_this_turn {
            self.temp_costs.energy = None;
            self.flags.zero_cost_this_turn = false;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerInstance {
    pub id: PowerInstanceId,
    pub def: PowerId,
    pub owner: CreatureId,
    pub amount: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelicInstance {
    pub id: RelicInstanceId,
    pub def: RelicId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PotionInstance {
    pub id: PotionInstanceId,
    pub def: PotionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creature {
    pub id: CreatureId,
    pub model: Option<MonsterId>,
    pub side: Side,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub powers: Vec<PowerInstanceId>,
    pub alive: bool,
    pub turns_taken: u32,
}

impl Creature {
    pub fn new(id: CreatureId, side: Side, max_hp: i32) -> Self {
        Self {
            id,
            model: None,
            side,
            hp: max_hp,
            max_hp,
            block: 0,
            powers: Vec::new(),
            alive: true,
            turns_taken: 0,
        }
    }

    pub fn with_model(mut self, model: MonsterId) -> Self {
        self.model = Some(model);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CardPiles {
    pub draw: Vec<CardInstanceId>,
    pub hand: Vec<CardInstanceId>,
    pub discard: Vec<CardInstanceId>,
    pub exhaust: Vec<CardInstanceId>,
    pub limbo: Vec<CardInstanceId>,
    pub play: Vec<CardInstanceId>,
    pub removed: Vec<CardInstanceId>,
}

impl CardPiles {
    pub fn pile(&self, kind: PileKind) -> &[CardInstanceId] {
        match kind {
            PileKind::Draw => &self.draw,
            PileKind::Hand => &self.hand,
            PileKind::Discard => &self.discard,
            PileKind::Exhaust => &self.exhaust,
            PileKind::Limbo => &self.limbo,
            PileKind::Play => &self.play,
            PileKind::Removed => &self.removed,
        }
    }

    pub fn pile_mut(&mut self, kind: PileKind) -> &mut Vec<CardInstanceId> {
        match kind {
            PileKind::Draw => &mut self.draw,
            PileKind::Hand => &mut self.hand,
            PileKind::Discard => &mut self.discard,
            PileKind::Exhaust => &mut self.exhaust,
            PileKind::Limbo => &mut self.limbo,
            PileKind::Play => &mut self.play,
            PileKind::Removed => &mut self.removed,
        }
    }

    pub fn remove(&mut self, card: CardInstanceId) -> Option<PileKind> {
        for kind in [
            PileKind::Draw,
            PileKind::Hand,
            PileKind::Discard,
            PileKind::Exhaust,
            PileKind::Limbo,
            PileKind::Play,
            PileKind::Removed,
        ] {
            let pile = self.pile_mut(kind);
            if let Some(index) = pile.iter().position(|existing| *existing == card) {
                pile.remove(index);
                return Some(kind);
            }
        }
        None
    }

    pub fn push(&mut self, kind: PileKind, card: CardInstanceId) {
        self.pile_mut(kind).push(card);
    }

    pub fn all_cards_in_pile_order(&self) -> Vec<CardInstanceId> {
        let mut out = Vec::new();
        for kind in [
            PileKind::Draw,
            PileKind::Hand,
            PileKind::Discard,
            PileKind::Exhaust,
            PileKind::Limbo,
            PileKind::Play,
            PileKind::Removed,
        ] {
            out.extend(self.pile(kind).iter().copied());
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerState {
    pub id: PlayerId,
    pub creature: CreatureId,
    pub energy: i32,
    pub max_energy: i32,
    pub stars: i32,
    pub relics: Vec<RelicInstanceId>,
    pub potions: Vec<PotionInstanceId>,
    pub piles: CardPiles,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatState {
    pub id: CombatId,
    pub phase: CombatPhase,
    pub player: PlayerState,
    pub creatures: Vec<Creature>,
    pub cards: BTreeMap<CardInstanceId, CardInstance>,
    pub powers: BTreeMap<PowerInstanceId, PowerInstance>,
    pub relics: BTreeMap<RelicInstanceId, RelicInstance>,
    pub potions: BTreeMap<PotionInstanceId, PotionInstance>,
    pub modifiers: Vec<ModifierInstanceId>,
    pub turn_stats: CombatTurnStats,
    pub combat_stats: CombatStats,
    next_card_instance: u32,
    next_power_instance: u32,
}

impl CombatState {
    pub fn monster_ids(&self) -> Vec<CreatureId> {
        self.creatures
            .iter()
            .filter(|creature| creature.side == Side::Monsters)
            .map(|creature| creature.id)
            .collect()
    }

    fn alloc_power_instance_id(&mut self) -> PowerInstanceId {
        let id = PowerInstanceId::new(self.next_power_instance);
        self.next_power_instance += 1;
        id
    }

    fn alloc_card_instance_id(&mut self) -> CardInstanceId {
        let id = CardInstanceId::new(self.next_card_instance);
        self.next_card_instance += 1;
        id
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatTurnStats {
    pub attacks_played: u32,
    pub cards_exhausted: u32,
    pub hp_lost_by_player: i32,
    pub card_block_gains: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatStats {
    pub hp_loss_events_by_player: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatSetupCard {
    pub def: CardId,
    pub upgraded: bool,
    pub costs: CardCosts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatSetupMonster {
    pub model: Option<MonsterId>,
    pub max_hp: i32,
}

#[derive(Clone, Debug)]
pub struct GameState {
    pub run: RunState,
    pub combat: Option<CombatState>,
    pub rng: RngSet,
}

impl GameState {
    pub fn new(seed: u64) -> Self {
        Self {
            run: RunState { seed },
            combat: None,
            rng: RngSet::seeded(seed),
        }
    }

    pub fn with_combat(seed: u64, combat: CombatState) -> Self {
        Self {
            run: RunState { seed },
            combat: Some(combat),
            rng: RngSet::seeded(seed),
        }
    }

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

    pub fn combat(&self) -> Option<&CombatState> {
        self.combat.as_ref()
    }

    pub fn combat_mut(&mut self) -> Option<&mut CombatState> {
        self.combat.as_mut()
    }

    pub fn player_id(&self) -> Option<PlayerId> {
        self.combat.as_ref().map(|combat| combat.player.id)
    }

    pub fn player_creature_id(&self) -> Option<CreatureId> {
        self.combat.as_ref().map(|combat| combat.player.creature)
    }

    pub fn creature(&self, id: CreatureId) -> Option<&Creature> {
        self.combat
            .as_ref()?
            .creatures
            .iter()
            .find(|creature| creature.id == id)
    }

    pub fn creature_mut(&mut self, id: CreatureId) -> Option<&mut Creature> {
        self.combat
            .as_mut()?
            .creatures
            .iter_mut()
            .find(|creature| creature.id == id)
    }

    pub fn card(&self, id: CardInstanceId) -> Option<&CardInstance> {
        self.combat.as_ref()?.cards.get(&id)
    }

    pub fn card_mut(&mut self, id: CardInstanceId) -> Option<&mut CardInstance> {
        self.combat.as_mut()?.cards.get_mut(&id)
    }

    pub fn card_is_in_pile(&self, id: CardInstanceId, pile: PileKind) -> bool {
        self.card(id)
            .map(|card| card.pile.kind == pile)
            .unwrap_or(false)
    }

    pub fn alive_monster_ids(&self) -> Vec<CreatureId> {
        self.combat
            .as_ref()
            .map(|combat| {
                combat
                    .creatures
                    .iter()
                    .filter(|creature| creature.side == Side::Monsters && creature.alive)
                    .map(|creature| creature.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn power_amount(&self, owner: CreatureId, power: PowerId) -> i32 {
        self.combat
            .as_ref()
            .and_then(|combat| {
                self.creature(owner).map(|creature| {
                    creature
                        .powers
                        .iter()
                        .filter_map(|id| combat.powers.get(id))
                        .filter(|instance| instance.def == power)
                        .map(|instance| instance.amount)
                        .sum()
                })
            })
            .unwrap_or(0)
    }

    pub fn has_power(&self, owner: CreatureId, power: PowerId) -> bool {
        self.power_amount(owner, power) != 0
    }

    pub fn resource_amount(
        &self,
        player: PlayerId,
        resource: ResourceKind,
    ) -> Result<i32, StateError> {
        let combat = self.combat.as_ref().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        Ok(match resource {
            ResourceKind::Energy => combat.player.energy,
            ResourceKind::Stars => combat.player.stars,
        })
    }

    pub fn spend_resource(
        &mut self,
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    ) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        if amount < 0 {
            return Err(StateError::InvalidResourceAmount { resource, amount });
        }
        let available = match resource {
            ResourceKind::Energy => combat.player.energy,
            ResourceKind::Stars => combat.player.stars,
        };
        if available < amount {
            return Err(StateError::NotEnoughResource {
                player,
                resource,
                available,
                required: amount,
            });
        }
        match resource {
            ResourceKind::Energy => combat.player.energy -= amount,
            ResourceKind::Stars => combat.player.stars -= amount,
        }
        Ok(())
    }

    pub fn gain_resource(
        &mut self,
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    ) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        if amount < 0 {
            return Err(StateError::InvalidResourceAmount { resource, amount });
        }
        match resource {
            ResourceKind::Energy => combat.player.energy += amount,
            ResourceKind::Stars => combat.player.stars += amount,
        }
        Ok(())
    }

    pub fn gain_block(&mut self, target: CreatureId, amount: Decimal) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.block;
        let gained = decimal_to_i32_trunc(amount);
        creature.block = creature.block.saturating_add(gained).min(999_999_999);
        Ok(creature.block - before)
    }

    pub fn lose_block(&mut self, target: CreatureId, amount: i32) -> Result<i32, StateError> {
        let block_loss = amount.max(0);
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.block;
        creature.block = creature.block.saturating_sub(block_loss);
        Ok(before - creature.block)
    }

    pub fn lose_hp(&mut self, target: CreatureId, amount: Decimal) -> Result<i32, StateError> {
        let hp_loss = decimal_to_i32_trunc(amount).max(0);
        let player = self.player_creature_id();
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.hp;
        creature.hp = creature.hp.saturating_sub(hp_loss).max(0);
        let actual = before - creature.hp;
        if actual > 0 && Some(target) == player {
            self.record_player_hp_loss(actual);
        }
        Ok(actual)
    }

    pub fn heal(&mut self, target: CreatureId, amount: Decimal) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.hp;
        let amount = decimal_to_i32_trunc(amount).max(0);
        creature.hp = creature.hp.saturating_add(amount).min(creature.max_hp);
        Ok(creature.hp - before)
    }

    pub fn gain_max_hp(&mut self, target: CreatureId, amount: i32) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let amount = amount.max(0);
        creature.max_hp = creature.max_hp.saturating_add(amount);
        creature.hp = creature.hp.saturating_add(amount).min(creature.max_hp);
        Ok(amount)
    }

    pub fn move_card(
        &mut self,
        card: CardInstanceId,
        to: PileId,
    ) -> Result<Option<PileKind>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let (owner, current_pile) = combat
            .cards
            .get(&card)
            .map(|card| (card.owner, card.pile))
            .ok_or(StateError::UnknownCard(card))?;

        if owner != combat.player.id || to.owner != owner {
            return Err(StateError::UnknownPlayer(owner));
        }

        let from = combat.player.piles.remove(card).or(Some(current_pile.kind));
        combat.player.piles.push(to.kind, card);

        if let Some(card_state) = combat.cards.get_mut(&card) {
            card_state.pile = to;
            if !matches!(to.kind, PileKind::Hand) {
                card_state.clear_turn_limited_state();
            }
        }

        Ok(from)
    }

    pub fn add_generated_card(
        &mut self,
        player: PlayerId,
        def: CardId,
        to: PileId,
        upgraded: bool,
        costs: CardCosts,
        temporary: bool,
        zero_cost_this_turn: bool,
    ) -> Result<CardInstanceId, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player || to.owner != player {
            return Err(StateError::UnknownPlayer(player));
        }

        let id = combat.alloc_card_instance_id();
        combat.cards.insert(
            id,
            CardInstance {
                id,
                def,
                owner: player,
                upgraded,
                costs,
                temp_costs: if zero_cost_this_turn {
                    TemporaryCardCosts {
                        energy: Some(CardCost::Fixed(0)),
                        stars: None,
                    }
                } else {
                    TemporaryCardCosts::default()
                },
                pile: to,
                flags: CardFlags {
                    temporary,
                    zero_cost_this_turn,
                    ..CardFlags::default()
                },
                counters: BTreeMap::new(),
            },
        );
        combat.player.piles.push(to.kind, id);
        Ok(id)
    }

    pub fn exhaust_card(&mut self, card: CardInstanceId) -> Result<Option<PileKind>, StateError> {
        let owner = self.card(card).ok_or(StateError::UnknownCard(card))?.owner;
        self.move_card(card, PileId::player(owner, PileKind::Exhaust))
    }

    pub fn upgrade_card(&mut self, card: CardInstanceId) -> Result<bool, StateError> {
        let card = self.card_mut(card).ok_or(StateError::UnknownCard(card))?;
        if card.upgraded {
            return Ok(false);
        }
        card.upgraded = true;
        Ok(true)
    }

    pub fn add_card_counter(
        &mut self,
        card: CardInstanceId,
        counter: CardCounter,
        amount: i32,
    ) -> Result<i32, StateError> {
        let card = self.card_mut(card).ok_or(StateError::UnknownCard(card))?;
        let value = card.counters.entry(counter).or_insert(0);
        *value += amount;
        Ok(*value)
    }

    pub fn shuffle_discard_into_draw_if_needed(
        &mut self,
        player: PlayerId,
    ) -> Result<Option<Vec<CardInstanceId>>, StateError> {
        let rng = &mut self.rng.shuffle;
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        if !combat.player.piles.draw.is_empty() || combat.player.piles.discard.is_empty() {
            return Ok(None);
        }

        let mut cards = std::mem::take(&mut combat.player.piles.discard);
        rng.shuffle(&mut cards);
        for card in &cards {
            if let Some(card_state) = combat.cards.get_mut(card) {
                card_state.pile = PileId::player(player, PileKind::Draw);
            }
        }
        combat.player.piles.draw = cards.clone();
        Ok(Some(cards))
    }

    pub fn draw_one_card(
        &mut self,
        player: PlayerId,
    ) -> Result<Option<CardInstanceId>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        if combat.player.piles.hand.len() >= MAX_CARDS_IN_HAND {
            return Ok(None);
        }

        let Some(card) = combat.player.piles.draw.pop() else {
            return Ok(None);
        };
        combat.player.piles.hand.push(card);
        if let Some(card_state) = combat.cards.get_mut(&card) {
            card_state.pile = PileId::player(player, PileKind::Hand);
        }
        Ok(Some(card))
    }

    pub fn draw_cards(
        &mut self,
        player: PlayerId,
        count: u8,
    ) -> Result<Vec<CardInstanceId>, StateError> {
        let mut drawn = Vec::new();
        for _ in 0..count {
            self.shuffle_discard_into_draw_if_needed(player)?;
            let Some(card) = self.draw_one_card(player)? else {
                break;
            };
            drawn.push(card);
        }
        Ok(drawn)
    }

    pub fn discard_hand(&mut self, player: PlayerId) -> Result<Vec<CardInstanceId>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        let cards = std::mem::take(&mut combat.player.piles.hand);
        for card in &cards {
            combat.player.piles.discard.push(*card);
            if let Some(card_state) = combat.cards.get_mut(card) {
                card_state.pile = PileId::player(player, PileKind::Discard);
            }
        }

        Ok(cards)
    }

    pub fn clear_side_block(&mut self, side: Side) -> Result<Vec<(CreatureId, i32)>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let mut cleared = Vec::new();

        for creature in combat
            .creatures
            .iter_mut()
            .filter(|creature| creature.side == side && creature.alive)
        {
            if creature.block > 0 {
                let amount = creature.block;
                creature.block = 0;
                cleared.push((creature.id, amount));
            }
        }

        Ok(cleared)
    }

    pub fn clear_block(&mut self, target: CreatureId) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let amount = creature.block;
        creature.block = 0;
        Ok(amount)
    }

    pub fn increment_turns_taken(&mut self, creature: CreatureId) -> Result<(), StateError> {
        let creature = self
            .creature_mut(creature)
            .ok_or(StateError::UnknownCreature(creature))?;
        creature.turns_taken += 1;
        Ok(())
    }

    pub fn apply_power(
        &mut self,
        target: CreatureId,
        power: PowerId,
        amount: Decimal,
    ) -> Result<(PowerInstanceId, i32), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let amount = decimal_to_i32_trunc(amount);
        let target_index = combat
            .creatures
            .iter()
            .position(|creature| creature.id == target)
            .ok_or(StateError::UnknownCreature(target))?;

        let existing = combat.creatures[target_index]
            .powers
            .iter()
            .copied()
            .find(|power_id| {
                combat
                    .powers
                    .get(power_id)
                    .map(|instance| instance.def == power)
                    .unwrap_or(false)
            });

        if let Some(existing) = existing {
            if let Some(instance) = combat.powers.get_mut(&existing) {
                instance.amount += amount;
            }
            return Ok((existing, amount));
        }

        let id = combat.alloc_power_instance_id();
        combat.powers.insert(
            id,
            PowerInstance {
                id,
                def: power,
                owner: target,
                amount,
            },
        );
        combat.creatures[target_index].powers.push(id);
        Ok((id, amount))
    }

    pub fn remove_power(&mut self, power: PowerInstanceId) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let Some(instance) = combat.powers.remove(&power) else {
            return Ok(());
        };
        if let Some(creature) = combat
            .creatures
            .iter_mut()
            .find(|creature| creature.id == instance.owner)
        {
            creature.powers.retain(|existing| *existing != power);
        }
        Ok(())
    }

    pub fn death_candidates(&self) -> Vec<CreatureId> {
        self.combat
            .as_ref()
            .map(|combat| {
                combat
                    .creatures
                    .iter()
                    .filter(|creature| creature.alive && creature.hp <= 0)
                    .map(|creature| creature.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn mark_dead(&mut self, creature: CreatureId) -> Result<(), StateError> {
        let target = self
            .creature_mut(creature)
            .ok_or(StateError::UnknownCreature(creature))?;
        target.alive = false;
        Ok(())
    }

    pub fn set_phase(&mut self, phase: CombatPhase) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        combat.phase = phase;
        Ok(())
    }

    pub fn reset_turn_stats(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats = CombatTurnStats::default();
        }
    }

    pub fn record_attack_played(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.attacks_played += 1;
        }
    }

    pub fn record_card_exhausted(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.cards_exhausted += 1;
        }
    }

    pub fn record_card_block_gained(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.card_block_gains += 1;
        }
    }

    pub fn record_player_hp_loss(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.hp_lost_by_player += amount;
            combat.combat_stats.hp_loss_events_by_player += 1;
        }
    }
}

pub(crate) fn decimal_to_i32_trunc(value: Decimal) -> i32 {
    i32::try_from(value.trunc()).unwrap_or_else(|_| {
        if value < Decimal::from(0) {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    CombatNotActive,
    UnknownPlayer(PlayerId),
    UnknownCreature(CreatureId),
    UnknownCard(CardInstanceId),
    InvalidResourceAmount {
        resource: ResourceKind,
        amount: i32,
    },
    NotEnoughResource {
        player: PlayerId,
        resource: ResourceKind,
        available: i32,
        required: i32,
    },
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CombatNotActive => write!(f, "combat is not active"),
            Self::UnknownPlayer(player) => write!(f, "unknown player: {:?}", player),
            Self::UnknownCreature(creature) => write!(f, "unknown creature: {:?}", creature),
            Self::UnknownCard(card) => write!(f, "unknown card: {:?}", card),
            Self::InvalidResourceAmount { resource, amount } => {
                write!(f, "invalid {:?} amount: {amount}", resource)
            }
            Self::NotEnoughResource {
                player,
                resource,
                available,
                required,
            } => write!(
                f,
                "player {:?} has {available} {:?}, but {required} is required",
                player, resource
            ),
        }
    }
}

impl std::error::Error for StateError {}
