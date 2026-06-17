use std::collections::BTreeMap;
use std::fmt;

use rust_decimal::Decimal;

use crate::core::ids::{
    CardId, CardInstanceId, CombatId, CreatureId, ModifierInstanceId, MonsterId, OrbId,
    OrbInstanceId, PlayerId, PotionId, PotionInstanceId, PowerId, PowerInstanceId, RelicId,
    RelicInstanceId,
};
use crate::core::rng::RngSet;

pub const MAX_CARDS_IN_HAND: usize = 10;
pub const BASE_HAND_DRAW_COUNT: u8 = 5;
pub const MAX_ORB_SLOTS: u8 = 10;

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
pub enum PlayerPetKind {
    Osty,
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
    pub retain_this_turn: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardCounter {
    DamageIncrease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PowerCounter {
    PanacheCardsLeft,
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

    pub(crate) fn clear_turn_limited_state(&mut self) {
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
    pub counters: BTreeMap<PowerCounter, i32>,
}

impl PowerInstance {
    pub fn counter(&self, counter: PowerCounter) -> Option<i32> {
        self.counters.get(&counter).copied()
    }
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
    pub pet_owner: Option<PlayerId>,
    pub pet_kind: Option<PlayerPetKind>,
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
            pet_owner: None,
            pet_kind: None,
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

    pub fn with_pet(mut self, owner: PlayerId, kind: PlayerPetKind) -> Self {
        self.pet_owner = Some(owner);
        self.pet_kind = Some(kind);
        self
    }

    /// HP can reach zero before death effects finish resolving; such creatures
    /// should already be skipped for targeting and further damage.
    pub fn is_hittable(&self) -> bool {
        self.alive && self.hp > 0
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

    pub fn remove_from(&mut self, kind: PileKind, card: CardInstanceId) -> bool {
        let pile = self.pile_mut(kind);
        if let Some(index) = pile.iter().position(|existing| *existing == card) {
            pile.remove(index);
            true
        } else {
            false
        }
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
    pub orb_queue: OrbQueue,
    pub relics: Vec<RelicInstanceId>,
    pub potions: Vec<PotionInstanceId>,
    pub piles: CardPiles,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrbQueue {
    pub base_slots: u8,
    pub slots: u8,
    pub orbs: Vec<OrbInstanceId>,
}

impl OrbQueue {
    pub fn with_base_slots(base_slots: u8) -> Self {
        let slots = base_slots.min(MAX_ORB_SLOTS);
        Self {
            base_slots: slots,
            slots,
            orbs: Vec::new(),
        }
    }

    pub fn has_room(&self) -> bool {
        self.orbs.len() < usize::from(self.slots)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbInstance {
    pub id: OrbInstanceId,
    pub def: OrbId,
    pub owner: PlayerId,
    pub amount: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatState {
    pub id: CombatId,
    pub phase: CombatPhase,
    pub player: PlayerState,
    pub creatures: Vec<Creature>,
    pub cards: BTreeMap<CardInstanceId, CardInstance>,
    pub powers: BTreeMap<PowerInstanceId, PowerInstance>,
    pub orbs: BTreeMap<OrbInstanceId, OrbInstance>,
    pub relics: BTreeMap<RelicInstanceId, RelicInstance>,
    pub potions: BTreeMap<PotionInstanceId, PotionInstance>,
    pub modifiers: Vec<ModifierInstanceId>,
    pub turn_stats: CombatTurnStats,
    pub combat_stats: CombatStats,
    pub(crate) next_card_instance: u32,
    pub(crate) next_power_instance: u32,
    pub(crate) next_orb_instance: u32,
}

impl CombatState {
    pub fn monster_ids(&self) -> Vec<CreatureId> {
        self.creatures
            .iter()
            .filter(|creature| creature.side == Side::Monsters)
            .map(|creature| creature.id)
            .collect()
    }

    pub(crate) fn alloc_power_instance_id(&mut self) -> PowerInstanceId {
        let id = PowerInstanceId::new(self.next_power_instance);
        self.next_power_instance += 1;
        id
    }

    pub(crate) fn alloc_orb_instance_id(&mut self) -> OrbInstanceId {
        let id = OrbInstanceId::new(self.next_orb_instance);
        self.next_orb_instance += 1;
        id
    }

    pub(crate) fn alloc_card_instance_id(&mut self) -> CardInstanceId {
        let id = CardInstanceId::new(self.next_card_instance);
        self.next_card_instance += 1;
        id
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatTurnStats {
    pub attacks_played: u32,
    pub cards_played: u32,
    pub cards_exhausted: u32,
    pub hp_lost_by_player: i32,
    pub card_block_gains: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatStats {
    pub hp_loss_events_by_player: u32,
    pub lightning_orbs_channeled: u32,
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
    UnknownPower(PowerInstanceId),
    UnknownOrb(OrbInstanceId),
    CardMissingFromPile {
        card: CardInstanceId,
        pile: PileId,
    },
    InvalidResourceAmount {
        resource: ResourceKind,
        amount: i32,
    },
    NoOrbSlot(PlayerId),
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
            Self::UnknownPower(power) => write!(f, "unknown power: {:?}", power),
            Self::UnknownOrb(orb) => write!(f, "unknown orb: {:?}", orb),
            Self::CardMissingFromPile { card, pile } => {
                write!(f, "card {:?} is missing from pile {:?}", card, pile)
            }
            Self::InvalidResourceAmount { resource, amount } => {
                write!(f, "invalid {:?} amount: {amount}", resource)
            }
            Self::NoOrbSlot(player) => write!(f, "player {:?} has no available orb slot", player),
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
