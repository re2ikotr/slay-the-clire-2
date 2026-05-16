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
}

impl CardInstance {
    pub fn effective_costs(&self) -> CardCosts {
        CardCosts {
            energy: self.temp_costs.energy.unwrap_or(self.costs.energy),
            stars: self.temp_costs.stars.unwrap_or(self.costs.stars),
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    pub seed: u64,
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

    pub fn card_is_in_pile(&self, id: CardInstanceId, pile: PileKind) -> bool {
        self.card(id)
            .map(|card| card.pile.kind == pile)
            .unwrap_or(false)
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
        }

        Ok(from)
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
