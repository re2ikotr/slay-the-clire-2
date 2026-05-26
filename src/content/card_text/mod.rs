use crate::content::cards::{CardKeyword, CardPoolId, CardType};
use crate::content::powers::CORRUPTION_POWER;
use crate::core::ids::{CardInstanceId, CreatureId};
use crate::core::query::ResourceCostCalc;
use crate::core::rules::RulePipeline;
use crate::core::state::{CardCost, CardCosts, GameState, ResourceKind};
use crate::registry::StaticRegistry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTextScope {
    Hand,
    Pile,
}

#[derive(Clone, Copy)]
pub struct CardTextCtx<'a> {
    pub state: &'a GameState,
    pub registry: &'a StaticRegistry,
    pub target: Option<CreatureId>,
    pub scope: CardTextScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardText {
    pub lines: Vec<CardTextLine>,
    pub keywords: Vec<CardKeyword>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardTextLine {
    pub eng: String,
    pub zhs: String,
}

pub fn describe_card(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> CardText {
    let lines = describe_lines(ctx, card);
    let keywords = describe_keywords(ctx, card);
    CardText { lines, keywords }
}

pub fn display_costs(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> CardCosts {
    let Some(card_state) = ctx.state.card(card) else {
        return CardCosts::default();
    };
    let def_costs = ctx
        .registry
        .cards
        .get(card_state.def)
        .map(|def| def.costs_for(card_state.upgraded))
        .unwrap_or(card_state.costs);
    let mut costs = card_state.costs_with_temporary(def_costs);
    if ctx.scope != CardTextScope::Hand {
        return costs;
    }
    let Some(player) = ctx.state.player_id() else {
        return costs;
    };
    costs.energy = display_resource_cost(ctx, player, card, ResourceKind::Energy, costs.energy);
    costs.stars = display_resource_cost(ctx, player, card, ResourceKind::Stars, costs.stars);
    costs
}

fn display_resource_cost(
    ctx: &CardTextCtx<'_>,
    player: crate::core::ids::PlayerId,
    card: CardInstanceId,
    resource: ResourceKind,
    cost: CardCost,
) -> CardCost {
    let CardCost::Fixed(base_cost) = cost else {
        return cost;
    };
    let calc = ResourceCostCalc {
        player,
        card,
        resource,
        base_cost,
        cost: base_cost,
    };
    let (calc, _) = RulePipeline::modify_resource_cost(ctx.registry, ctx.state, calc);
    CardCost::Fixed(calc.cost.max(0))
}

fn describe_keywords(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardKeyword> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    let Some(def) = ctx.registry.cards.get(card_state.def) else {
        return Vec::new();
    };
    let mut keywords = Vec::new();
    for keyword in def.keywords {
        push_keyword(&mut keywords, *keyword);
    }
    if card_state.upgraded {
        for keyword in def.upgraded_keywords {
            push_keyword(&mut keywords, *keyword);
        }
    }
    if card_state.flags.ethereal {
        push_keyword(&mut keywords, CardKeyword::Ethereal);
    }
    if card_state.flags.temporary {
        push_keyword(&mut keywords, CardKeyword::Temporary);
    }
    if card_state.flags.purge_on_use {
        push_keyword(&mut keywords, CardKeyword::PurgeOnUse);
    }
    if card_state.flags.zero_cost_this_turn {
        push_keyword(&mut keywords, CardKeyword::FreeThisTurn);
    }
    if ctx.scope == CardTextScope::Hand
        && def.card_type == CardType::Skill
        && ctx
            .state
            .player_creature_id()
            .map(|creature| ctx.state.has_power(creature, CORRUPTION_POWER))
            .unwrap_or(false)
    {
        push_keyword(&mut keywords, CardKeyword::Exhaust);
    }
    keywords
}

fn describe_lines(ctx: &CardTextCtx<'_>, card: CardInstanceId) -> Vec<CardTextLine> {
    let Some(card_state) = ctx.state.card(card) else {
        return Vec::new();
    };
    let Some(def) = ctx.registry.cards.get(card_state.def) else {
        return Vec::new();
    };
    match def.pool {
        CardPoolId::Ironclad => ironclad::describe_lines(ctx, card),
        CardPoolId::Colorless => colorless::describe_lines(ctx, card),
        CardPoolId::Curse => curses::describe_lines(ctx, card),
        CardPoolId::Defect => defect::describe_lines(ctx, card),
        CardPoolId::Event => events::describe_lines(ctx, card),
        CardPoolId::Necrobinder => necrobinder::describe_lines(ctx, card),
        CardPoolId::Quest => quests::describe_lines(ctx, card),
        CardPoolId::Regent => regent::describe_lines(ctx, card),
        CardPoolId::Silent => silent::describe_lines(ctx, card),
        CardPoolId::Status => statuses::describe_lines(ctx, card),
        CardPoolId::Token => tokens::describe_lines(ctx, card),
        CardPoolId::Deprecated => Vec::new(),
    }
}

fn push_keyword(keywords: &mut Vec<CardKeyword>, keyword: CardKeyword) {
    if !keywords.contains(&keyword) {
        keywords.push(keyword);
    }
}

pub mod ironclad;

pub mod colorless;
pub mod curses;
pub mod defect;
pub mod events;
pub mod necrobinder;
pub mod quests;
pub mod regent;
pub mod silent;
pub mod statuses;
pub mod tokens;
