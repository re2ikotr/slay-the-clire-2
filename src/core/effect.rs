use rust_decimal::Decimal;

use crate::content::cards::{CardType, TargetType};
use crate::core::event::Event;
use crate::core::ids::{
    CardId, CardInstanceId, ChoiceId, CreatureId, LocKey, OrbId, OrbInstanceId, PlayerId,
    PotionInstanceId, PowerId, PowerInstanceId, RelicInstanceId,
};
use crate::core::state::{
    CardCounter, CombatPhase, PileId, PileKind, PowerCounter, ResourceKind, Side,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Trigger(Event),
    ValidateCardPlay {
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
    },
    SpendResource {
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    },
    GainResource {
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    },
    PayCardCosts {
        player: PlayerId,
        card: CardInstanceId,
    },
    ExecuteCardBody {
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
    },
    PrepareCardPlayResult {
        player: PlayerId,
        card: CardInstanceId,
        force_exhaust: bool,
    },
    FinishCardPlay {
        player: PlayerId,
        card: CardInstanceId,
        target: Option<CreatureId>,
        force_exhaust: bool,
    },
    DealDamage(DamageOp),
    DealDamageToAllEnemies(DamageAllEnemiesOp),
    DealDamageToRandomEnemy(RandomDamageOp),
    LoseHp {
        target: CreatureId,
        amount: Decimal,
        source: Option<Source>,
    },
    Heal {
        target: CreatureId,
        amount: Decimal,
        source: Option<Source>,
    },
    GainMaxHp {
        target: CreatureId,
        amount: i32,
        source: Option<Source>,
    },
    GainMaxHpIfFatal {
        target: CreatureId,
        beneficiary: CreatureId,
        amount: i32,
        source: Option<Source>,
    },
    GainBlock {
        target: CreatureId,
        amount: Decimal,
        source: Option<Source>,
    },
    ApplyPower {
        target: CreatureId,
        power: PowerId,
        amount: Decimal,
        source: Option<Source>,
    },
    ApplyPowerToRandomEnemy {
        power: PowerId,
        amount: Decimal,
        source: Option<Source>,
        count: u8,
    },
    AddPowerAmount {
        power: PowerInstanceId,
        amount: Decimal,
        source: Option<Source>,
    },
    RemovePower {
        power: PowerInstanceId,
    },
    DrawCards {
        player: PlayerId,
        count: u8,
    },
    DrawHandCards {
        player: PlayerId,
        count: u8,
    },
    DrawUntilNonAttack {
        player: PlayerId,
    },
    DiscardHand {
        player: PlayerId,
        kind: DiscardKind,
    },
    DiscardCards {
        player: PlayerId,
        cards: Vec<CardInstanceId>,
        kind: DiscardKind,
        then_draw: u8,
    },
    ExhaustCard {
        card: CardInstanceId,
    },
    ExhaustTopDraw {
        player: PlayerId,
        count: u8,
    },
    ExhaustRandomHand {
        player: PlayerId,
        filter: CardFilter,
    },
    ExhaustHand {
        player: PlayerId,
        filter: CardFilter,
    },
    UpgradeCard {
        card: CardInstanceId,
    },
    UpgradeHand {
        player: PlayerId,
        mode: UpgradeMode,
    },
    RetainCardsThisTurn {
        cards: Vec<CardInstanceId>,
    },
    AddCardCounter {
        card: CardInstanceId,
        counter: CardCounter,
        amount: i32,
    },
    SetPowerCounter {
        power: PowerInstanceId,
        counter: PowerCounter,
        value: i32,
    },
    AddGeneratedCard {
        player: PlayerId,
        def: CardId,
        to: PileId,
        upgraded: bool,
        temporary: bool,
        zero_cost_this_turn: bool,
    },
    GenerateRandomCardToHand {
        player: PlayerId,
        card_type: Option<CardType>,
        target: Option<TargetType>,
        zero_cost_this_turn: bool,
    },
    DiscoverRandomCardsToHand {
        player: PlayerId,
        count: u8,
        zero_cost_this_turn: bool,
    },
    PlayTopDrawCards {
        player: PlayerId,
        count: u8,
        exhaust_after_play: bool,
    },
    PlayRandomCardsFromPile {
        player: PlayerId,
        pile: PileKind,
        filter: CardFilter,
        count: u8,
        exhaust_after_play: bool,
    },
    AddOrbSlots {
        player: PlayerId,
        amount: u8,
    },
    RemoveOrbSlots {
        player: PlayerId,
        amount: u8,
    },
    ChannelOrb {
        player: PlayerId,
        orb: OrbId,
        source: Option<Source>,
    },
    ChannelRandomOrb {
        player: PlayerId,
        source: Option<Source>,
    },
    EvokeOrb {
        player: PlayerId,
        target: OrbSelection,
        remove: bool,
        source: Option<Source>,
    },
    TriggerOrbPassive {
        orb: OrbInstanceId,
        trigger: OrbTrigger,
        target: Option<CreatureId>,
    },
    AddOrbAmount {
        orb: OrbInstanceId,
        amount: i32,
    },
    SummonOsty {
        player: PlayerId,
        amount: Decimal,
        source: Option<Source>,
    },
    KillCreature {
        creature: CreatureId,
        source: Option<Source>,
    },
    ClearSideBlock(Side),
    ExecuteMonsterTurn,
    MoveCard {
        card: CardInstanceId,
        to: PileId,
        reason: MoveReason,
    },
    SelectHandCards {
        player: PlayerId,
        filter: CardFilter,
        min: usize,
        max: usize,
        prompt: LocKey,
        source: Option<Source>,
        on_resolve: ChoiceAction,
    },
    SelectPileCards {
        player: PlayerId,
        pile: PileKind,
        filter: CardFilter,
        min: usize,
        max: usize,
        prompt: LocKey,
        source: Option<Source>,
        on_resolve: ChoiceAction,
    },
    CheckDeaths,
    CheckCombatEnd,
    StartTurn(Side),
    EndTurn(Side),
    EnterPhase(CombatPhase),
    RequestChoice(ChoiceRequest),
    ResolveChoice(ChoiceResolution),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Card(CardInstanceId),
    Power(PowerInstanceId),
    Relic(RelicInstanceId),
    Potion(PotionInstanceId),
    Creature(CreatureId),
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscardKind {
    Manual,
    EndOfTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageKind {
    Attack,
    Power,
    Thorns,
    LifeLoss,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageFlags {
    pub ignores_block: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageOp {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub base_amount: Decimal,
    pub kind: DamageKind,
    pub flags: DamageFlags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageAllEnemiesOp {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub base_amount: Decimal,
    pub kind: DamageKind,
    pub flags: DamageFlags,
    pub hit_count: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomDamageOp {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub base_amount: Decimal,
    pub kind: DamageKind,
    pub flags: DamageFlags,
    pub hit_count: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamageResult {
    pub source: Option<Source>,
    pub dealer: Option<CreatureId>,
    pub target: CreatureId,
    pub kind: DamageKind,
    pub requested: Decimal,
    pub blocked: i32,
    pub hp_loss: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbSelection {
    First,
    Last,
    Exact(OrbInstanceId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbTrigger {
    AfterTurnStart,
    BeforeTurnEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveReason {
    Draw,
    Discard,
    Exhaust,
    Play,
    Generated,
    Cleanup,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardFilter {
    Any,
    Attack,
    NonAttack,
    NotRetainedThisTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeMode {
    First,
    All,
    Random,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceRequest {
    pub id: ChoiceId,
    pub kind: ChoiceKind,
    pub source: Option<Source>,
    pub prompt: LocKey,
    pub min: usize,
    pub max: usize,
    pub on_resolve: ChoiceAction,
    pub options: Vec<ChoiceOption>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceKind {
    SelectCard,
    SelectTarget,
    SelectReward,
    Generic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceOption {
    pub id: ChoiceId,
    pub loc_key: LocKey,
    pub value: ChoiceValue,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceValue {
    Card(CardInstanceId),
    CardDef(CardId),
    Target(CreatureId),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceAction {
    None,
    ExhaustSelectedCards,
    DiscardSelectedCards,
    DiscardSelectedCardsThenDraw(u8),
    DiscardSelectedCardsThenAddCard {
        def: CardId,
        count: u8,
        upgraded: bool,
    },
    MoveSelectedCardsToPile {
        pile: PileKind,
        reason: MoveReason,
    },
    RetainSelectedCardsThisTurn,
    AddSelectedCardDefsToHand {
        upgraded: bool,
        temporary: bool,
        zero_cost_this_turn: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceResponse {
    pub request: ChoiceId,
    pub options: Vec<ChoiceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceResolution {
    pub request: ChoiceRequest,
    pub selected: Vec<ChoiceOption>,
}
