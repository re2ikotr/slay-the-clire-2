use rust_decimal::Decimal;

use crate::content::cards::{CardTag, CardType};
use crate::core::effect::{DamageKind, Effect, Source};
use crate::core::event::Event;
use crate::core::ids::{LocKey, PowerId, PowerInstanceId};
use crate::core::query::{
    BlockCalc, CardPlayResultPileCalc, DamageCalc, Decision, DecisionQuery, DecisionQueryKind,
    PreventReason, ResourceCostCalc,
};
use crate::core::rules::{prevent_by_current_listener, RuleCtx};
use crate::core::state::{PileId, PileKind, ResourceKind, Side};
use crate::registry::DefRegistry;

pub type PowerEventFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, &Event) -> Vec<Effect>;
pub type PowerModifyDamageFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, DamageCalc) -> DamageCalc;
pub type PowerModifyBlockFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, BlockCalc) -> BlockCalc;
pub type PowerModifyResourceCostFn =
    for<'a> fn(&RuleCtx<'a>, PowerInstanceId, ResourceCostCalc) -> ResourceCostCalc;
pub type PowerModifyResultPileFn =
    for<'a> fn(&RuleCtx<'a>, PowerInstanceId, CardPlayResultPileCalc) -> CardPlayResultPileCalc;
pub type PowerDecisionFn = for<'a> fn(&RuleCtx<'a>, PowerInstanceId, &DecisionQuery) -> Decision;

#[derive(Clone)]
pub struct PowerDef {
    pub id: PowerId,
    pub loc_key: LocKey,
    pub rules: PowerRules,
}

#[derive(Clone, Default)]
pub struct PowerRules {
    pub on_event: Option<PowerEventFn>,
    pub modify_damage_additive: Option<PowerModifyDamageFn>,
    pub modify_damage_multiplicative: Option<PowerModifyDamageFn>,
    pub modify_damage_cap: Option<PowerModifyDamageFn>,
    pub modify_block_additive: Option<PowerModifyBlockFn>,
    pub modify_block_multiplicative: Option<PowerModifyBlockFn>,
    pub modify_resource_cost: Option<PowerModifyResourceCostFn>,
    pub modify_card_play_result_pile: Option<PowerModifyResultPileFn>,
    pub decide: Option<PowerDecisionFn>,
}

macro_rules! power_ids {
    ($($name:ident => $id:literal,)*) => {
        $(pub const $name: PowerId = PowerId::new($id);)*
    };
}

power_ids! {
    STRENGTH => "STRENGTH",
    VULNERABLE => "VULNERABLE",
    WEAK => "WEAK",
    PLATING_POWER => "PLATING",
    AGGRESSION_POWER => "AGGRESSION_POWER",
    BARRICADE_POWER => "BARRICADE_POWER",
    COLOSSUS_POWER => "COLOSSUS_POWER",
    CORRUPTION_POWER => "CORRUPTION_POWER",
    CRIMSON_MANTLE_POWER => "CRIMSON_MANTLE_POWER",
    CRUELTY_POWER => "CRUELTY_POWER",
    DARK_EMBRACE_POWER => "DARK_EMBRACE_POWER",
    DEMON_FORM_POWER => "DEMON_FORM_POWER",
    DRUM_OF_BATTLE_POWER => "DRUM_OF_BATTLE_POWER",
    FEEL_NO_PAIN_POWER => "FEEL_NO_PAIN_POWER",
    FLAME_BARRIER_POWER => "FLAME_BARRIER_POWER",
    FREE_ATTACK_POWER => "FREE_ATTACK_POWER",
    HELLRAISER_POWER => "HELLRAISER_POWER",
    INFERNO_POWER => "INFERNO_POWER",
    JUGGERNAUT_POWER => "JUGGERNAUT_POWER",
    JUGGLING_POWER => "JUGGLING_POWER",
    MANGLE_POWER => "MANGLE_POWER",
    NO_DRAW_POWER => "NO_DRAW_POWER",
    NO_ENERGY_GAIN_POWER => "NO_ENERGY_GAIN_POWER",
    ONE_TWO_PUNCH_POWER => "ONE_TWO_PUNCH_POWER",
    PYRE_POWER => "PYRE_POWER",
    RAGE_POWER => "RAGE_POWER",
    RUPTURE_POWER => "RUPTURE_POWER",
    SETUP_STRIKE_POWER => "SETUP_STRIKE_POWER",
    STAMPEDE_POWER => "STAMPEDE_POWER",
    TANK_POWER => "TANK_POWER",
    UNMOVABLE_POWER => "UNMOVABLE_POWER",
    VICIOUS_POWER => "VICIOUS_POWER",
}

pub fn register_core_powers(registry: &mut DefRegistry<PowerId, PowerDef>) {
    for def in [
        strength(),
        vulnerable(),
        weak(),
        plating(),
        simple_event_power(AGGRESSION_POWER, aggression_on_event),
        barricade(),
        colossus(),
        corruption(),
        simple_event_power(CRIMSON_MANTLE_POWER, crimson_mantle_on_event),
        cruelty(),
        simple_event_power(DARK_EMBRACE_POWER, dark_embrace_on_event),
        simple_event_power(DEMON_FORM_POWER, demon_form_on_event),
        simple_event_power(DRUM_OF_BATTLE_POWER, drum_of_battle_on_event),
        simple_event_power(FEEL_NO_PAIN_POWER, feel_no_pain_on_event),
        simple_event_power(FLAME_BARRIER_POWER, flame_barrier_on_event),
        free_attack(),
        simple_event_power(HELLRAISER_POWER, hellraiser_on_event),
        simple_event_power(INFERNO_POWER, inferno_on_event),
        simple_event_power(JUGGERNAUT_POWER, juggernaut_on_event),
        simple_event_power(JUGGLING_POWER, juggling_on_event),
        simple_event_power(MANGLE_POWER, temporary_strength_loss_on_event),
        no_draw(),
        no_energy_gain(),
        simple_event_power(ONE_TWO_PUNCH_POWER, one_two_punch_on_event),
        simple_event_power(PYRE_POWER, pyre_on_event),
        simple_event_power(RAGE_POWER, rage_on_event),
        simple_event_power(RUPTURE_POWER, rupture_on_event),
        simple_event_power(SETUP_STRIKE_POWER, temporary_strength_gain_on_event),
        simple_event_power(STAMPEDE_POWER, stampede_on_event),
        tank(),
        unmovable(),
        simple_event_power(VICIOUS_POWER, vicious_on_event),
    ] {
        registry.register(def);
    }
}

pub fn strength() -> PowerDef {
    PowerDef {
        id: STRENGTH,
        loc_key: LocKey::new("power.strength"),
        rules: PowerRules {
            modify_damage_additive: Some(strength_modify_damage_additive),
            ..PowerRules::default()
        },
    }
}

fn vulnerable() -> PowerDef {
    PowerDef {
        id: VULNERABLE,
        loc_key: LocKey::new("power.vulnerable"),
        rules: PowerRules {
            modify_damage_multiplicative: Some(vulnerable_modify_damage),
            ..PowerRules::default()
        },
    }
}

fn weak() -> PowerDef {
    PowerDef {
        id: WEAK,
        loc_key: LocKey::new("power.weak"),
        rules: PowerRules {
            modify_damage_multiplicative: Some(weak_modify_damage),
            ..PowerRules::default()
        },
    }
}

fn plating() -> PowerDef {
    simple_event_power(PLATING_POWER, plating_on_event)
}

fn barricade() -> PowerDef {
    PowerDef {
        id: BARRICADE_POWER,
        loc_key: LocKey::new("power.barricade"),
        rules: PowerRules {
            decide: Some(barricade_decide),
            ..PowerRules::default()
        },
    }
}

fn colossus() -> PowerDef {
    PowerDef {
        id: COLOSSUS_POWER,
        loc_key: LocKey::new("power.colossus"),
        rules: PowerRules {
            on_event: Some(remove_on_player_turn_end),
            modify_damage_multiplicative: Some(colossus_modify_damage),
            ..PowerRules::default()
        },
    }
}

fn corruption() -> PowerDef {
    PowerDef {
        id: CORRUPTION_POWER,
        loc_key: LocKey::new("power.corruption"),
        rules: PowerRules {
            modify_resource_cost: Some(corruption_modify_cost),
            modify_card_play_result_pile: Some(corruption_modify_result_pile),
            ..PowerRules::default()
        },
    }
}

fn cruelty() -> PowerDef {
    PowerDef {
        id: CRUELTY_POWER,
        loc_key: LocKey::new("power.cruelty"),
        rules: PowerRules {
            modify_damage_multiplicative: Some(cruelty_modify_damage),
            ..PowerRules::default()
        },
    }
}

fn free_attack() -> PowerDef {
    PowerDef {
        id: FREE_ATTACK_POWER,
        loc_key: LocKey::new("power.free_attack"),
        rules: PowerRules {
            on_event: Some(free_attack_on_event),
            modify_resource_cost: Some(free_attack_modify_cost),
            ..PowerRules::default()
        },
    }
}

fn no_draw() -> PowerDef {
    PowerDef {
        id: NO_DRAW_POWER,
        loc_key: LocKey::new("power.no_draw"),
        rules: PowerRules {
            on_event: Some(remove_on_player_turn_end),
            decide: Some(no_draw_decide),
            ..PowerRules::default()
        },
    }
}

fn no_energy_gain() -> PowerDef {
    simple_event_power(NO_ENERGY_GAIN_POWER, remove_on_player_turn_end)
}

fn tank() -> PowerDef {
    PowerDef {
        id: TANK_POWER,
        loc_key: LocKey::new("power.tank"),
        rules: PowerRules {
            modify_damage_multiplicative: Some(tank_modify_damage),
            ..PowerRules::default()
        },
    }
}

fn unmovable() -> PowerDef {
    PowerDef {
        id: UNMOVABLE_POWER,
        loc_key: LocKey::new("power.unmovable"),
        rules: PowerRules {
            modify_block_multiplicative: Some(unmovable_modify_block),
            ..PowerRules::default()
        },
    }
}

fn simple_event_power(id: PowerId, on_event: PowerEventFn) -> PowerDef {
    PowerDef {
        id,
        loc_key: LocKey::new(id.as_str()),
        rules: PowerRules {
            on_event: Some(on_event),
            ..PowerRules::default()
        },
    }
}

fn strength_modify_damage_additive(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };

    if calc.kind == DamageKind::Attack && calc.dealer == Some(instance.owner) {
        calc.amount += Decimal::from(instance.amount);
    }

    if calc.amount < Decimal::from(0) {
        calc.amount = Decimal::from(0);
    }

    calc
}

fn vulnerable_modify_damage(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    if calc.kind == DamageKind::Attack && calc.target == instance.owner {
        calc.amount *= Decimal::new(15, 1);
    }
    calc
}

fn weak_modify_damage(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    if calc.kind == DamageKind::Attack && calc.dealer == Some(instance.owner) {
        calc.amount *= Decimal::new(75, 2);
    }
    calc
}

fn colossus_modify_damage(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    if calc.target == instance.owner
        && calc
            .dealer
            .map(|dealer| ctx.state.power_amount(dealer, VULNERABLE) > 0)
            .unwrap_or(false)
    {
        calc.amount *= Decimal::new(5, 1);
    }
    calc
}

fn cruelty_modify_damage(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    if calc.kind == DamageKind::Attack
        && ctx.state.power_amount(calc.target, VULNERABLE) > 0
        && ctx
            .state
            .creature(instance.owner)
            .map(|c| c.side == Side::Player)
            .unwrap_or(false)
    {
        calc.amount *= Decimal::from(100 + instance.amount) / Decimal::from(100);
    }
    calc
}

fn tank_modify_damage(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: DamageCalc,
) -> DamageCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    if calc.target == instance.owner
        && calc
            .dealer
            .and_then(|dealer| ctx.state.creature(dealer))
            .map(|dealer| dealer.side == Side::Monsters)
            .unwrap_or(false)
    {
        calc.amount *= Decimal::from(2);
    }
    calc
}

fn unmovable_modify_block(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: BlockCalc,
) -> BlockCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    let already_gained = ctx
        .state
        .combat()
        .map(|combat| combat.turn_stats.card_block_gains > 0)
        .unwrap_or(false);
    if calc.target == instance.owner
        && matches!(calc.source, Some(Source::Card(_)))
        && !already_gained
    {
        calc.amount *= Decimal::from(2);
    }
    calc
}

fn corruption_modify_cost(
    ctx: &RuleCtx<'_>,
    _power: PowerInstanceId,
    mut calc: ResourceCostCalc,
) -> ResourceCostCalc {
    let is_skill = ctx
        .state
        .card(calc.card)
        .and_then(|card| ctx.registry.cards.get(card.def))
        .map(|def| def.card_type == CardType::Skill)
        .unwrap_or(false);
    if calc.resource == ResourceKind::Energy && is_skill {
        calc.cost = 0;
    }
    calc
}

fn corruption_modify_result_pile(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    mut calc: CardPlayResultPileCalc,
) -> CardPlayResultPileCalc {
    let Some(instance) = power_instance(ctx, power) else {
        return calc;
    };
    let Some(card) = ctx.state.card(calc.card) else {
        return calc;
    };
    let card_owner_creature = ctx
        .state
        .combat()
        .and_then(|combat| (combat.player.id == card.owner).then_some(combat.player.creature));
    let is_skill = ctx
        .registry
        .cards
        .get(card.def)
        .map(|def| def.card_type == CardType::Skill)
        .unwrap_or(false);
    if card_owner_creature == Some(instance.owner) && is_skill {
        calc.pile = PileId::player(card.owner, PileKind::Exhaust);
    }
    calc
}

fn free_attack_modify_cost(
    ctx: &RuleCtx<'_>,
    _power: PowerInstanceId,
    mut calc: ResourceCostCalc,
) -> ResourceCostCalc {
    if calc.resource != ResourceKind::Energy {
        return calc;
    }
    let is_attack = ctx
        .state
        .card(calc.card)
        .and_then(|card| ctx.registry.cards.get(card.def))
        .map(|def| def.card_type == CardType::Attack)
        .unwrap_or(false);
    if is_attack {
        calc.cost = 0;
    }
    calc
}

fn barricade_decide(ctx: &RuleCtx<'_>, power: PowerInstanceId, query: &DecisionQuery) -> Decision {
    let Some(instance) = power_instance(ctx, power) else {
        return Decision::Allow;
    };
    match query.kind {
        DecisionQueryKind::ShouldClearBlock { creature } if creature == instance.owner => {
            prevent_by_current_listener(ctx, PreventReason::Custom("barricade"))
        }
        _ => Decision::Allow,
    }
}

fn no_draw_decide(ctx: &RuleCtx<'_>, _power: PowerInstanceId, query: &DecisionQuery) -> Decision {
    if matches!(query.kind, DecisionQueryKind::ShouldDraw { .. }) {
        prevent_by_current_listener(ctx, PreventReason::CannotDraw)
    } else {
        Decision::Allow
    }
}

fn aggression_on_event(ctx: &RuleCtx<'_>, _power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    if !matches!(event, Event::TurnStarted { side: Side::Player }) {
        return Vec::new();
    }
    let Some(combat) = ctx.state.combat() else {
        return Vec::new();
    };
    let Some(card) = combat.player.piles.discard.last().copied() else {
        return Vec::new();
    };
    vec![
        Effect::MoveCard {
            card,
            to: crate::core::state::PileId::player(
                combat.player.id,
                crate::core::state::PileKind::Hand,
            ),
            reason: crate::core::effect::MoveReason::Generated,
        },
        Effect::UpgradeCard { card },
    ]
}

fn crimson_mantle_on_event(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    event: &Event,
) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::TurnStarted { side: Side::Player }) {
        vec![
            Effect::LoseHp {
                target: instance.owner,
                amount: Decimal::from(1),
                source: Some(Source::Power(power)),
            },
            Effect::GainBlock {
                target: instance.owner,
                amount: Decimal::from(instance.amount),
                source: Some(Source::Power(power)),
            },
        ]
    } else {
        Vec::new()
    }
}

fn dark_embrace_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    let Event::CardExhausted(event) = event else {
        return Vec::new();
    };
    let Some(player) = ctx.state.combat().map(|combat| combat.player.id) else {
        return Vec::new();
    };
    if ctx.state.player_creature_id() == Some(instance.owner) && event.player == player {
        vec![Effect::DrawCards {
            player,
            count: instance.amount.max(1) as u8,
        }]
    } else {
        Vec::new()
    }
}

fn demon_form_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    turn_start_gain_strength(ctx, power, event)
}

fn pyre_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if !matches!(event, Event::TurnStarted { side: Side::Player }) {
        return Vec::new();
    }
    ctx.state
        .player_id()
        .map(|player| {
            vec![Effect::GainResource {
                player,
                resource: ResourceKind::Energy,
                amount: instance.amount,
            }]
        })
        .unwrap_or_default()
}

fn drum_of_battle_on_event(
    ctx: &RuleCtx<'_>,
    _power: PowerInstanceId,
    event: &Event,
) -> Vec<Effect> {
    if !matches!(event, Event::TurnStarted { side: Side::Player }) {
        return Vec::new();
    }
    ctx.state
        .player_id()
        .map(|player| vec![Effect::ExhaustTopDraw { player, count: 1 }])
        .unwrap_or_default()
}

fn feel_no_pain_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::CardExhausted(_)) {
        vec![Effect::GainBlock {
            target: instance.owner,
            amount: Decimal::from(instance.amount),
            source: Some(Source::Power(power)),
        }]
    } else {
        Vec::new()
    }
}

fn flame_barrier_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    match event {
        Event::DamageDealt(result) if result.target == instance.owner && result.hp_loss > 0 => {
            result
                .dealer
                .map(|dealer| {
                    vec![Effect::DealDamage(crate::core::effect::DamageOp {
                        source: Some(Source::Power(power)),
                        dealer: Some(instance.owner),
                        target: dealer,
                        base_amount: Decimal::from(instance.amount),
                        kind: DamageKind::Thorns,
                        flags: crate::core::effect::DamageFlags {
                            ignores_block: false,
                        },
                    })]
                })
                .unwrap_or_default()
        }
        Event::TurnEnded { side: Side::Player } => vec![Effect::RemovePower { power }],
        _ => Vec::new(),
    }
}

fn hellraiser_on_event(ctx: &RuleCtx<'_>, _power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Event::CardDrawn(event) = event else {
        return Vec::new();
    };
    let Some(card) = ctx.state.card(event.card) else {
        return Vec::new();
    };
    let is_strike = ctx
        .registry
        .cards
        .get(card.def)
        .map(|def| def.has_tag(CardTag::Strike))
        .unwrap_or(false);
    if !is_strike {
        return Vec::new();
    }
    let target = ctx.state.alive_monster_ids().first().copied();
    vec![Effect::ExecuteCardBody {
        player: event.player,
        card: event.card,
        target,
    }]
}

fn inferno_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    match event {
        Event::TurnStarted { side: Side::Player } => vec![Effect::LoseHp {
            target: instance.owner,
            amount: Decimal::from(1),
            source: Some(Source::Power(power)),
        }],
        Event::CreatureHpChanged(event)
            if event.creature == instance.owner && event.before > event.after =>
        {
            vec![Effect::DealDamageToAllEnemies(
                crate::core::effect::DamageAllEnemiesOp {
                    source: Some(Source::Power(power)),
                    dealer: Some(instance.owner),
                    base_amount: Decimal::from(instance.amount),
                    kind: DamageKind::Power,
                    flags: crate::core::effect::DamageFlags {
                        ignores_block: false,
                    },
                    hit_count: 1,
                },
            )]
        }
        _ => Vec::new(),
    }
}

fn juggernaut_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::BlockGained(event) if event.target == instance.owner && event.amount > 0)
    {
        vec![Effect::DealDamageToRandomEnemy(
            crate::core::effect::RandomDamageOp {
                source: Some(Source::Power(power)),
                dealer: Some(instance.owner),
                base_amount: Decimal::from(instance.amount),
                kind: DamageKind::Power,
                flags: crate::core::effect::DamageFlags {
                    ignores_block: false,
                },
                hit_count: 1,
            },
        )]
    } else {
        Vec::new()
    }
}

fn juggling_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(_instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    let Event::CardPlayed(event) = event else {
        return Vec::new();
    };
    let attacks = ctx
        .state
        .combat()
        .map(|combat| combat.turn_stats.attacks_played)
        .unwrap_or(0);
    if attacks == 3 {
        ctx.state
            .card(event.card)
            .map(|card| {
                vec![Effect::AddGeneratedCard {
                    player: event.player,
                    def: card.def,
                    to: crate::core::state::PileId::player(
                        event.player,
                        crate::core::state::PileKind::Hand,
                    ),
                    upgraded: card.upgraded,
                    temporary: true,
                    zero_cost_this_turn: false,
                }]
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn one_two_punch_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if instance.amount <= 0 {
        return vec![Effect::RemovePower { power }];
    }
    let Event::CardPlayed(event) = event else {
        return remove_on_player_turn_end(ctx, power, event);
    };
    let is_attack = ctx
        .state
        .card(event.card)
        .and_then(|card| ctx.registry.cards.get(card.def))
        .map(|def| def.card_type == CardType::Attack)
        .unwrap_or(false);
    if is_attack {
        vec![
            Effect::ExecuteCardBody {
                player: event.player,
                card: event.card,
                target: event.target,
            },
            Effect::ApplyPower {
                target: instance.owner,
                power: ONE_TWO_PUNCH_POWER,
                amount: Decimal::from(-1),
                source: Some(Source::Power(power)),
            },
        ]
    } else {
        Vec::new()
    }
}

fn rage_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    match event {
        Event::CardPlayed(event) => {
            let is_attack = ctx
                .state
                .card(event.card)
                .and_then(|card| ctx.registry.cards.get(card.def))
                .map(|def| def.card_type == CardType::Attack)
                .unwrap_or(false);
            if is_attack {
                vec![Effect::GainBlock {
                    target: instance.owner,
                    amount: Decimal::from(instance.amount),
                    source: Some(Source::Power(power)),
                }]
            } else {
                Vec::new()
            }
        }
        Event::TurnEnded { side: Side::Player } => vec![Effect::RemovePower { power }],
        _ => Vec::new(),
    }
}

fn rupture_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::CreatureHpChanged(event) if event.creature == instance.owner && event.before > event.after)
    {
        vec![Effect::ApplyPower {
            target: instance.owner,
            power: STRENGTH,
            amount: Decimal::from(instance.amount),
            source: Some(Source::Power(power)),
        }]
    } else {
        Vec::new()
    }
}

fn stampede_on_event(ctx: &RuleCtx<'_>, _power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    if !matches!(event, Event::TurnEnded { side: Side::Player }) {
        return Vec::new();
    }
    let Some(combat) = ctx.state.combat() else {
        return Vec::new();
    };
    let Some(card) = combat.player.piles.hand.first().copied() else {
        return Vec::new();
    };
    let target = ctx.state.alive_monster_ids().first().copied();
    vec![Effect::ExecuteCardBody {
        player: combat.player.id,
        card,
        target,
    }]
}

fn vicious_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    let Event::PowerApplied(event) = event else {
        return Vec::new();
    };
    if event.power == VULNERABLE && event.amount > 0 {
        ctx.state
            .player_id()
            .map(|player| {
                vec![Effect::DrawCards {
                    player,
                    count: instance.amount as u8,
                }]
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn plating_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::TurnStarted { side: Side::Player }) {
        vec![Effect::GainBlock {
            target: instance.owner,
            amount: Decimal::from(instance.amount),
            source: Some(Source::Power(power)),
        }]
    } else {
        Vec::new()
    }
}

fn free_attack_on_event(ctx: &RuleCtx<'_>, power: PowerInstanceId, event: &Event) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if instance.amount <= 0 {
        return vec![Effect::RemovePower { power }];
    }
    if matches!(event, Event::CardPlayed(_)) {
        if instance.amount <= 1 {
            vec![Effect::RemovePower { power }]
        } else {
            vec![Effect::ApplyPower {
                target: instance.owner,
                power: FREE_ATTACK_POWER,
                amount: Decimal::from(-1),
                source: Some(Source::Power(power)),
            }]
        }
    } else {
        remove_on_player_turn_end(ctx, power, event)
    }
}

fn temporary_strength_gain_on_event(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    event: &Event,
) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::TurnEnded { side: Side::Player }) {
        vec![
            Effect::ApplyPower {
                target: instance.owner,
                power: STRENGTH,
                amount: Decimal::from(-instance.amount),
                source: Some(Source::Power(power)),
            },
            Effect::RemovePower { power },
        ]
    } else {
        Vec::new()
    }
}

fn temporary_strength_loss_on_event(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    event: &Event,
) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::TurnEnded { side: Side::Player }) {
        vec![
            Effect::ApplyPower {
                target: instance.owner,
                power: STRENGTH,
                amount: Decimal::from(instance.amount),
                source: Some(Source::Power(power)),
            },
            Effect::RemovePower { power },
        ]
    } else {
        Vec::new()
    }
}

fn turn_start_gain_strength(
    ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    event: &Event,
) -> Vec<Effect> {
    let Some(instance) = power_instance(ctx, power) else {
        return Vec::new();
    };
    if matches!(event, Event::TurnStarted { side: Side::Player }) {
        vec![Effect::ApplyPower {
            target: instance.owner,
            power: STRENGTH,
            amount: Decimal::from(instance.amount),
            source: Some(Source::Power(power)),
        }]
    } else {
        Vec::new()
    }
}

fn remove_on_player_turn_end(
    _ctx: &RuleCtx<'_>,
    power: PowerInstanceId,
    event: &Event,
) -> Vec<Effect> {
    if matches!(event, Event::TurnEnded { side: Side::Player }) {
        vec![Effect::RemovePower { power }]
    } else {
        Vec::new()
    }
}

fn power_instance<'a>(
    ctx: &'a RuleCtx<'_>,
    power: PowerInstanceId,
) -> Option<&'a crate::core::state::PowerInstance> {
    ctx.state.combat()?.powers.get(&power)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{CardDef, CardPlayCtx, CardRarity, CardRules, TargetType};
    use crate::core::event::CardDrawn;
    use crate::core::ids::{CardId, CardInstanceId, CreatureId};
    use crate::core::state::{CardCosts, CombatSetupCard, CombatSetupMonster, GameState};
    use crate::registry::StaticRegistry;

    const TEST_STRIKE_TAGS: &[CardTag] = &[CardTag::Strike];
    const TEST_NO_TAGS: &[CardTag] = &[];

    fn test_card_play(
        _: &CardPlayCtx<'_>,
        _: CardInstanceId,
        _: Option<CreatureId>,
    ) -> Vec<Effect> {
        Vec::new()
    }

    fn test_card_def(id: CardId, tags: &'static [CardTag]) -> CardDef {
        CardDef {
            id,
            loc_key: LocKey::new("card.test"),
            card_type: CardType::Attack,
            rarity: CardRarity::Common,
            target: TargetType::Enemy,
            base_costs: CardCosts::energy(1),
            upgraded_costs: None,
            keywords: &[],
            upgraded_keywords: &[],
            tags,
            can_generate_in_combat: true,
            play: test_card_play,
            rules: CardRules::default(),
        }
    }

    fn hellraiser_effects_for(card_def: CardDef) -> Vec<Effect> {
        let mut registry = StaticRegistry::empty();
        registry.cards.register(card_def.clone());
        let state = GameState::single_player_test_combat(
            1,
            [CombatSetupCard {
                def: card_def.id,
                upgraded: false,
                costs: CardCosts::energy(1),
            }],
            [CombatSetupMonster {
                model: None,
                max_hp: 30,
            }],
            80,
            3,
            1,
        );
        let combat = state.combat().expect("combat exists");
        let ctx = RuleCtx {
            state: &state,
            registry: &registry,
            listener: None,
        };

        hellraiser_on_event(
            &ctx,
            PowerInstanceId::new(1),
            &Event::CardDrawn(CardDrawn {
                player: combat.player.id,
                card: combat.player.piles.hand[0],
                from_hand_draw: false,
            }),
        )
    }

    #[test]
    fn hellraiser_uses_strike_tag_when_card_id_does_not_contain_strike() {
        let effects =
            hellraiser_effects_for(test_card_def(CardId::new("TEST_BLADE"), TEST_STRIKE_TAGS));

        assert!(matches!(
            effects.as_slice(),
            [Effect::ExecuteCardBody { .. }]
        ));
    }

    #[test]
    fn hellraiser_ignores_strike_named_cards_without_strike_tag() {
        let effects = hellraiser_effects_for(test_card_def(
            CardId::new("TEST_STRIKE_NAMED_ONLY"),
            TEST_NO_TAGS,
        ));

        assert!(effects.is_empty());
    }
}
