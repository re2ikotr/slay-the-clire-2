use std::collections::VecDeque;
use std::io::{self, Write};

mod animation;
mod profile;
mod ratatui_renderer;
mod symbols;
mod theme;

use crate::adapters::log_store::StepLogSink;
use crate::assets::{Language, Localization};
use crate::content::card_text::{describe_card, display_costs, CardTextCtx, CardTextScope};
use crate::content::cards::TargetType;
use crate::content::scenarios;
use crate::core::ids::{CardInstanceId, CreatureId};
use crate::core::rules::{RuleCtx, RulePipeline};
use crate::core::state::{
    CardCost, CardCosts, CombatPhase, PileKind, Side, BASE_HAND_DRAW_COUNT, MAX_CARDS_IN_HAND,
};
use crate::core::{Command, Engine, StepResult};
use crate::registry::StaticRegistry;

const DEMO_DECK_SIZE: usize = 25;
const DEFAULT_NIBBIT_COUNT: usize = 3;
const PLAYER_MAX_HP: i32 = 80;
const PLAYER_MAX_ENERGY: i32 = 3;
const MAX_MESSAGES: usize = 8;

pub fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let profile = profile::TerminalProfile::detect(&args);
    let seed = parse_seed(&args).unwrap_or(0);
    let language = parse_language(&args).unwrap_or(Language::Zhs);
    let nibbit_count = parse_nibbit_count(&args).unwrap_or(DEFAULT_NIBBIT_COUNT);

    let registry = StaticRegistry::standard();
    let driver = LocalCombatDriver::new(registry, seed, nibbit_count);

    if profile.use_plain_renderer() {
        let mut app = TuiApp::new(driver, language);
        let mut renderer = PlainRenderer;
        if let Err(error) = app.run(&mut renderer) {
            eprintln!("tui error: {error}");
        }
    } else {
        let mut app = ratatui_renderer::RatatuiCombatApp::new(driver, language, profile);
        if let Err(error) = app.run() {
            eprintln!("tui error: {error}");
        }
    }
}

fn parse_seed(args: &[String]) -> Option<u64> {
    args.iter()
        .find_map(|arg| arg.strip_prefix("--seed="))
        .and_then(|value| value.parse().ok())
}

fn parse_language(args: &[String]) -> Option<Language> {
    args.iter()
        .find_map(|arg| arg.strip_prefix("--lang="))
        .and_then(Language::from_code)
}

fn parse_nibbit_count(args: &[String]) -> Option<usize> {
    args.iter()
        .find_map(|arg| arg.strip_prefix("--nibbits="))
        .and_then(|value| value.parse().ok())
        .filter(|count| *count > 0)
}

struct TuiApp<D> {
    driver: D,
    loc: Localization,
    messages: VecDeque<String>,
}

impl<D: CombatDriver> TuiApp<D> {
    fn new(driver: D, language: Language) -> Self {
        let loc = Localization::new(language);
        let mut messages = VecDeque::new();
        messages.push_back(loc.ui("help.compact").to_string());
        Self {
            driver,
            loc,
            messages,
        }
    }

    fn run(&mut self, renderer: &mut dyn CombatRenderer) -> io::Result<()> {
        let mut input = String::new();
        loop {
            let snapshot = self.driver.snapshot(&self.loc);
            renderer.render(&snapshot, &self.messages, &self.loc)?;

            print!("> ");
            io::stdout().flush()?;
            input.clear();
            if io::stdin().read_line(&mut input)? == 0 {
                return Ok(());
            }

            match parse_input(input.trim(), &self.loc) {
                UiInput::Quit => return Ok(()),
                UiInput::Help => self.push_message(self.loc.ui("help.full").to_string()),
                UiInput::SetLanguage(language) => {
                    self.loc.set_language(language);
                    self.push_message(self.loc.format_language_changed());
                }
                UiInput::Restart => {
                    let result = self.driver.submit(CombatUiAction::Restart, &self.loc);
                    self.push_messages(result.messages);
                }
                UiInput::EndTurn => {
                    let result = self.driver.submit(CombatUiAction::EndTurn, &self.loc);
                    self.push_messages(result.messages);
                }
                UiInput::Play { hand, monster } => {
                    let result = self.driver.submit(
                        CombatUiAction::PlayHandCard {
                            hand_index: hand,
                            monster_index: monster,
                        },
                        &self.loc,
                    );
                    self.push_messages(result.messages);
                }
                UiInput::Empty => {}
                UiInput::Invalid(message) => self.push_message(message),
            }
        }
    }

    fn push_messages(&mut self, messages: Vec<String>) {
        for message in messages {
            self.push_message(message);
        }
    }

    fn push_message(&mut self, message: String) {
        if self.messages.len() == MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }
}

enum UiInput {
    Empty,
    Help,
    SetLanguage(Language),
    Quit,
    Restart,
    EndTurn,
    Play { hand: usize, monster: Option<usize> },
    Invalid(String),
}

fn parse_input(input: &str, loc: &Localization) -> UiInput {
    if input.is_empty() {
        return UiInput::Empty;
    }
    match input {
        "?" | "h" | "help" => return UiInput::Help,
        "q" | "quit" => return UiInput::Quit,
        "r" | "restart" => return UiInput::Restart,
        "e" | "end" | "endturn" => return UiInput::EndTurn,
        _ => {}
    }

    let mut parts = input.split_whitespace();
    let Some(card) = parts.next() else {
        return UiInput::Empty;
    };
    if matches!(card, "lang" | "language") {
        return match parts.next().and_then(Language::from_code) {
            Some(language) => UiInput::SetLanguage(language),
            None => UiInput::Invalid(loc.ui("error.language_usage").to_string()),
        };
    }
    let hand = match card.parse::<usize>() {
        Ok(0) => return UiInput::Invalid(loc.ui("error.card_index_starts_at_one").to_string()),
        Ok(value) => value - 1,
        Err(_) => return UiInput::Invalid(loc.format_unknown_command(input)),
    };
    let monster = match parts.next() {
        Some(value) => match value.parse::<usize>() {
            Ok(0) => {
                return UiInput::Invalid(loc.ui("error.monster_index_starts_at_one").to_string())
            }
            Ok(value) => Some(value - 1),
            Err(_) => return UiInput::Invalid(loc.format_invalid_monster_index(value)),
        },
        None => None,
    };
    UiInput::Play { hand, monster }
}

trait CombatDriver {
    fn snapshot(&self, loc: &Localization) -> CombatSnapshot;
    fn submit(&mut self, action: CombatUiAction, loc: &Localization) -> UiStepResult;
}

enum CombatUiAction {
    PlayHandCard {
        hand_index: usize,
        monster_index: Option<usize>,
    },
    EndTurn,
    Restart,
}

struct UiStepResult {
    messages: Vec<String>,
}

struct LocalCombatDriver {
    registry: StaticRegistry,
    engine: Engine,
    seed: u64,
    nibbit_count: usize,
    log_sink: Option<StepLogSink>,
}

impl LocalCombatDriver {
    fn new(registry: StaticRegistry, seed: u64, nibbit_count: usize) -> Self {
        let engine = build_demo_engine(&registry, seed, nibbit_count);
        Self {
            registry,
            engine,
            seed,
            nibbit_count,
            log_sink: create_log_sink("tui"),
        }
    }

    fn restart(&mut self) {
        self.seed = self.seed.wrapping_add(1);
        self.engine = build_demo_engine(&self.registry, self.seed, self.nibbit_count);
        self.record_note("restart", &format!("seed: {}", self.seed));
    }

    fn record_step(&mut self, label: &str, result: &StepResult) {
        if let Some(sink) = self.log_sink.as_mut() {
            if let Err(error) = sink.record_step(label, result) {
                eprintln!("failed to write step log: {error}");
                self.log_sink = None;
            }
        }
    }

    fn record_note(&mut self, label: &str, note: &str) {
        if let Some(sink) = self.log_sink.as_mut() {
            if let Err(error) = sink.record_note(label, note) {
                eprintln!("failed to write step log: {error}");
                self.log_sink = None;
            }
        }
    }
}

impl CombatDriver for LocalCombatDriver {
    fn snapshot(&self, loc: &Localization) -> CombatSnapshot {
        CombatSnapshot::from_engine(&self.engine, loc)
    }

    fn submit(&mut self, action: CombatUiAction, loc: &Localization) -> UiStepResult {
        match action {
            CombatUiAction::Restart => {
                self.restart();
                UiStepResult {
                    messages: vec![loc.format_started_test_combat(self.seed)],
                }
            }
            CombatUiAction::EndTurn => {
                let result = self.engine.step(Command::EndTurn { side: Side::Player });
                self.record_step(loc.ui("action.end_turn"), &result);
                UiStepResult {
                    messages: summarize_step(loc.ui("action.end_turn"), result, loc),
                }
            }
            CombatUiAction::PlayHandCard {
                hand_index,
                monster_index,
            } => {
                let Some(combat) = self.engine.state.combat() else {
                    return UiStepResult {
                        messages: vec![loc.ui("error.no_active_combat").to_string()],
                    };
                };
                if combat.phase != CombatPhase::PlayerAction {
                    return UiStepResult {
                        messages: vec![loc.ui("error.not_player_action_phase").to_string()],
                    };
                }
                let Some(card) = combat.player.piles.hand.get(hand_index).copied() else {
                    return UiStepResult {
                        messages: vec![loc.format_no_card_at_hand_index(hand_index + 1)],
                    };
                };
                let target = resolve_target(&self.engine, card, monster_index);
                let player = combat.player.id;
                let label = card_label(&self.engine, card, loc);
                let result = self.engine.step(Command::PlayCard {
                    player,
                    card,
                    target,
                });
                self.record_step(&loc.format_play_card(&label), &result);
                UiStepResult {
                    messages: summarize_step(&loc.format_play_card(&label), result, loc),
                }
            }
        }
    }
}

fn create_log_sink(session: &str) -> Option<StepLogSink> {
    match StepLogSink::create(session) {
        Ok(sink) => Some(sink),
        Err(error) => {
            eprintln!("log disabled: {error}");
            None
        }
    }
}

fn build_demo_engine(registry: &StaticRegistry, seed: u64, nibbit_count: usize) -> Engine {
    let state = scenarios::random_nibbit_combat(
        registry,
        seed,
        nibbit_count,
        DEMO_DECK_SIZE,
        PLAYER_MAX_HP,
        PLAYER_MAX_ENERGY,
        BASE_HAND_DRAW_COUNT,
    );
    Engine::with_registry(state, registry.clone())
}

fn resolve_target(
    engine: &Engine,
    card: CardInstanceId,
    monster_index: Option<usize>,
) -> Option<CreatureId> {
    let combat = engine.state.combat()?;
    let monster_ids = combat
        .creatures
        .iter()
        .filter(|creature| creature.side == Side::Monsters && creature.alive)
        .map(|creature| creature.id)
        .collect::<Vec<_>>();

    let explicit_monster = monster_index.and_then(|index| monster_ids.get(index).copied());
    let target_type = engine
        .state
        .card(card)
        .and_then(|card| engine.registry.cards.get(card.def))
        .map(|def| def.target);

    match target_type {
        Some(
            TargetType::None
            | TargetType::SelfTarget
            | TargetType::AnyPlayer
            | TargetType::AnyAlly
            | TargetType::AllAllies
            | TargetType::TargetedNoCreature
            | TargetType::Osty,
        ) => None,
        Some(TargetType::AnyCreature) => explicit_monster.or_else(|| monster_ids.first().copied()),
        Some(TargetType::Enemy) => explicit_monster.or_else(|| monster_ids.first().copied()),
        Some(TargetType::AllEnemies | TargetType::RandomEnemy) => None,
        None => explicit_monster,
    }
}

fn summarize_step(label: &str, result: StepResult, loc: &Localization) -> Vec<String> {
    match result {
        StepResult::Done(log) => {
            let mut messages = vec![format!(
                "{label}: {} ({} {})",
                loc.ui("status.done"),
                log.len(),
                loc.ui("unit.log_entries")
            )];
            messages.extend(summarize_log(log, loc));
            messages
        }
        StepResult::NeedChoice(choice, log) => vec![format!(
            "{label}: {} {:?} ({} {})",
            loc.ui("status.choice_requested"),
            choice.kind,
            log.len(),
            loc.ui("unit.log_entries")
        )],
        StepResult::CombatOver(result, log) => vec![format!(
            "{label}: {} {:?} ({} {})",
            loc.ui("status.combat_ended"),
            result.outcome,
            log.len(),
            loc.ui("unit.log_entries")
        )],
        StepResult::Rejected(error, log) => vec![format!(
            "{label}: {} ({} {}): {error}",
            loc.ui("status.rejected"),
            log.len(),
            loc.ui("unit.log_entries")
        )],
        StepResult::Failed(error, log) => {
            vec![format!(
                "{label}: {} ({} {}): {error}",
                loc.ui("status.failed"),
                log.len(),
                loc.ui("unit.log_entries")
            )]
        }
    }
}

fn summarize_log(log: Vec<crate::core::log::LogEntry>, loc: &Localization) -> Vec<String> {
    let mut out = Vec::new();
    for entry in log {
        match entry {
            crate::core::log::LogEntry::StateChanged(change) => match change {
                crate::core::log::StateChange::DamageApplied(result) if result.hp_loss > 0 => {
                    out.push(format!(
                        "{} {:?} -> {:?}: {} {}",
                        loc.ui("log.damage"),
                        result.dealer,
                        result.target,
                        result.hp_loss,
                        loc.ui("label.hp")
                    ));
                }
                crate::core::log::StateChange::BlockGained { target, amount } if amount > 0 => {
                    out.push(format!("{} {:?}: +{}", loc.ui("log.block"), target, amount));
                }
                crate::core::log::StateChange::CardMoved { reason, .. } => {
                    out.push(format!("{}: {reason:?}", loc.ui("log.card_moved")));
                }
                crate::core::log::StateChange::CardsShuffled { cards, .. } => {
                    out.push(format!(
                        "{} {} {}",
                        loc.ui("log.shuffled"),
                        cards.len(),
                        loc.ui("log.cards")
                    ));
                }
                crate::core::log::StateChange::CreatureDied { creature } => {
                    out.push(format!("{}: {:?}", loc.ui("log.creature_died"), creature));
                }
                _ => {}
            },
            crate::core::log::LogEntry::CombatEnded(result) => {
                out.push(format!(
                    "{}: {:?}",
                    loc.ui("log.combat_result"),
                    result.outcome
                ));
            }
            _ => {}
        }
        if out.len() >= 4 {
            break;
        }
    }
    out
}

trait CombatRenderer {
    fn render(
        &mut self,
        snapshot: &CombatSnapshot,
        messages: &VecDeque<String>,
        loc: &Localization,
    ) -> io::Result<()>;
}

struct PlainRenderer;

impl CombatRenderer for PlainRenderer {
    fn render(
        &mut self,
        snapshot: &CombatSnapshot,
        messages: &VecDeque<String>,
        loc: &Localization,
    ) -> io::Result<()> {
        println!();
        render_snapshot(snapshot, messages, loc)
    }
}

fn render_snapshot(
    snapshot: &CombatSnapshot,
    messages: &VecDeque<String>,
    loc: &Localization,
) -> io::Result<()> {
    println!(
        "{} | {} {} | {} {} | lang {}",
        loc.ui("app.title"),
        loc.ui("label.seed"),
        snapshot.seed,
        loc.ui("label.phase"),
        loc.phase(snapshot.phase),
        loc.language().code()
    );
    let star_suffix = if snapshot.stars > 0 {
        format!("  {} {}", loc.ui("label.stars"), snapshot.stars)
    } else {
        String::new()
    };
    println!(
        "{}: {} {}/{}{}  {} {}/{}{}",
        loc.ui("label.player"),
        loc.ui("label.hp"),
        snapshot.player.hp,
        snapshot.player.max_hp,
        block_suffix(snapshot.player.block, loc),
        loc.ui("label.energy"),
        snapshot.energy,
        snapshot.max_energy,
        star_suffix
    );
    if !snapshot.player.powers.is_empty() {
        println!(
            "{}: {}",
            loc.ui("label.status"),
            format_power_list(&snapshot.player.powers)
        );
    }
    println!(
        "{}: {} {}  {} {}  {} {}",
        loc.ui("label.piles"),
        loc.ui("label.draw"),
        snapshot.draw_pile.len(),
        loc.ui("label.discard"),
        snapshot.discard_pile.len(),
        loc.ui("label.exhaust"),
        snapshot.exhaust_pile.len()
    );
    println!();
    println!("{}:", loc.ui("label.monsters"));
    for (index, monster) in snapshot.monsters.iter().enumerate() {
        let dead_suffix = if monster.alive {
            String::new()
        } else {
            format!("  {}", loc.ui("label.dead"))
        };
        println!(
            "  {}. {}  {} {}/{}{}  {} {}{}",
            index + 1,
            monster.label,
            loc.ui("label.hp"),
            monster.hp,
            monster.max_hp,
            block_suffix(monster.block, loc),
            loc.ui("label.intent"),
            monster.intent,
            dead_suffix
        );
        if !monster.powers.is_empty() {
            println!(
                "     {}: {}",
                loc.ui("label.status"),
                format_power_list(&monster.powers)
            );
        }
    }
    if snapshot.monsters.is_empty() {
        println!("  {}", loc.ui("label.none"));
    }
    println!();
    println!(
        "{} {}/{}:",
        loc.ui("label.hand"),
        snapshot.hand.len(),
        MAX_CARDS_IN_HAND
    );
    for (index, card) in snapshot.hand.iter().enumerate() {
        let keywords = if card.keywords.is_empty() {
            String::new()
        } else {
            format!("  [{}]", card.keywords.join(", "))
        };
        println!(
            "  {}. {:<8} {}  {}{}",
            index + 1,
            card.cost,
            pad_display(&card.label, 28),
            card.card_type,
            keywords
        );
        for line in &card.description {
            println!("      {line}");
        }
    }
    if snapshot.hand.is_empty() {
        println!("  {}", loc.ui("label.empty"));
    }
    println!();
    println!("{}:", loc.ui("label.messages"));
    for message in messages {
        println!("  {message}");
    }
    io::stdout().flush()
}

fn pad_display(value: &str, min_width: usize) -> String {
    let width = display_width(value);
    if width >= min_width {
        value.to_string()
    } else {
        format!("{value}{}", " ".repeat(min_width - width))
    }
}

fn block_suffix(block: i32, loc: &Localization) -> String {
    if block > 0 {
        format!("  {} {}", loc.ui("label.block"), block)
    } else {
        String::new()
    }
}

fn display_width(value: &str) -> usize {
    value.chars().map(char_display_width).sum()
}

fn char_display_width(ch: char) -> usize {
    let code = ch as u32;
    if matches!(
        code,
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
    ) {
        2
    } else {
        1
    }
}

#[derive(Clone, Debug)]
struct CombatSnapshot {
    seed: u64,
    phase: CombatPhase,
    player: CreatureView,
    energy: i32,
    max_energy: i32,
    stars: i32,
    draw_pile: Vec<CardView>,
    discard_pile: Vec<CardView>,
    exhaust_pile: Vec<CardView>,
    monsters: Vec<CreatureView>,
    hand: Vec<CardView>,
}

impl CombatSnapshot {
    fn from_engine(engine: &Engine, loc: &Localization) -> Self {
        let combat = engine.state.combat();
        let phase = combat
            .map(|combat| combat.phase)
            .unwrap_or(CombatPhase::CombatStart);
        let player_creature = combat
            .and_then(|combat| engine.state.creature(combat.player.creature))
            .map(|creature| CreatureView::from_creature(engine, creature, loc))
            .unwrap_or_else(|| CreatureView::placeholder(loc.ui("entity.player"), loc));

        let visible_monster_ids = combat
            .map(|combat| {
                combat
                    .creatures
                    .iter()
                    .filter(|creature| creature.side == Side::Monsters)
                    .filter(|creature| monster_is_visible_in_snapshot(engine, creature))
                    .map(|creature| creature.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let monsters = visible_monster_ids
            .iter()
            .copied()
            .map(|creature| CreatureView::from_monster(engine, creature, loc))
            .collect::<Vec<_>>();

        let hand = combat
            .map(|combat| {
                combat
                    .player
                    .piles
                    .hand
                    .iter()
                    .copied()
                    .map(|card| CardView::from_card(engine, card, loc, Some(&visible_monster_ids)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Self {
            seed: engine.state.run.seed,
            phase,
            player: player_creature,
            energy: combat
                .map(|combat| combat.player.energy)
                .unwrap_or_default(),
            max_energy: combat
                .map(|combat| combat.player.max_energy)
                .unwrap_or_default(),
            stars: combat.map(|combat| combat.player.stars).unwrap_or_default(),
            draw_pile: pile_cards(combat, engine, loc, PileKind::Draw),
            discard_pile: pile_cards(combat, engine, loc, PileKind::Discard),
            exhaust_pile: pile_cards(combat, engine, loc, PileKind::Exhaust),
            monsters,
            hand,
        }
    }
}

fn pile_cards(
    combat: Option<&crate::core::state::CombatState>,
    engine: &Engine,
    loc: &Localization,
    pile: PileKind,
) -> Vec<CardView> {
    combat
        .map(|combat| {
            combat
                .player
                .piles
                .pile(pile)
                .iter()
                .copied()
                .map(|card| CardView::from_card(engine, card, loc, None))
                .collect()
        })
        .unwrap_or_default()
}

fn monster_is_visible_in_snapshot(
    engine: &Engine,
    creature: &crate::core::state::Creature,
) -> bool {
    creature.alive
        || !RulePipeline::should_remove_creature_after_death(
            &engine.registry,
            &engine.state,
            creature.id,
        )
        .is_allowed()
}

#[derive(Clone, Debug)]
struct CreatureView {
    label: String,
    hp: i32,
    max_hp: i32,
    block: i32,
    intent: String,
    powers: Vec<PowerView>,
    alive: bool,
}

impl CreatureView {
    fn placeholder(label: &str, loc: &Localization) -> Self {
        Self {
            label: label.to_string(),
            hp: 0,
            max_hp: 0,
            block: 0,
            intent: loc.ui("label.unknown").to_string(),
            powers: Vec::new(),
            alive: false,
        }
    }

    fn from_creature(
        engine: &Engine,
        creature: &crate::core::state::Creature,
        loc: &Localization,
    ) -> Self {
        Self {
            label: loc.ui("entity.player").to_string(),
            hp: creature.hp,
            max_hp: creature.max_hp,
            block: creature.block,
            intent: loc.ui("label.none").to_string(),
            powers: power_views(engine, creature.id, loc),
            alive: creature.alive,
        }
    }

    fn from_monster(engine: &Engine, id: CreatureId, loc: &Localization) -> Self {
        let Some(creature) = engine.state.creature(id) else {
            return Self::placeholder(loc.ui("entity.monster"), loc);
        };
        let label = creature
            .model
            .and_then(|model| engine.registry.monsters.get(model))
            .map(|def| loc.entity_name(def.loc_key))
            .unwrap_or_else(|| format!("{:?}", creature.id));
        Self {
            label,
            hp: creature.hp,
            max_hp: creature.max_hp,
            block: creature.block,
            intent: monster_intent_label(engine, id, loc),
            powers: power_views(engine, creature.id, loc),
            alive: creature.alive,
        }
    }
}

#[derive(Clone, Debug)]
struct PowerView {
    label: String,
    amount: i32,
}

fn power_views(engine: &Engine, owner: CreatureId, loc: &Localization) -> Vec<PowerView> {
    let Some(combat) = engine.state.combat() else {
        return Vec::new();
    };
    let Some(creature) = engine.state.creature(owner) else {
        return Vec::new();
    };

    creature
        .powers
        .iter()
        .filter_map(|power_id| combat.powers.get(power_id))
        .map(|instance| {
            let label = engine
                .registry
                .powers
                .get(instance.def)
                .map(|def| loc.entity_name(def.loc_key))
                .unwrap_or_else(|| instance.def.as_str().to_string());
            PowerView {
                label,
                amount: instance.amount,
            }
        })
        .collect()
}

fn format_power_list(powers: &[PowerView]) -> String {
    powers
        .iter()
        .map(format_power)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_power(power: &PowerView) -> String {
    if power.amount == 1 {
        power.label.clone()
    } else {
        format!("{} {}", power.label, power.amount)
    }
}

#[derive(Clone, Debug)]
struct CardView {
    label: String,
    card_type: String,
    cost: String,
    costs: CardCosts,
    target: TargetType,
    keywords: Vec<String>,
    description: Vec<String>,
    target_descriptions: Vec<Vec<String>>,
}

impl CardView {
    fn from_card(
        engine: &Engine,
        id: CardInstanceId,
        loc: &Localization,
        preview_targets: Option<&[CreatureId]>,
    ) -> Self {
        let Some(card) = engine.state.card(id) else {
            return Self {
                label: format!("{:?}", id),
                card_type: loc.ui("label.unknown").to_string(),
                cost: "?".to_string(),
                costs: CardCosts::default(),
                target: TargetType::None,
                keywords: Vec::new(),
                description: Vec::new(),
                target_descriptions: Vec::new(),
            };
        };
        let def = engine.registry.cards.get(card.def);
        let scope = if card.pile.kind == PileKind::Hand {
            CardTextScope::Hand
        } else {
            CardTextScope::Pile
        };
        let text_ctx = CardTextCtx {
            state: &engine.state,
            registry: &engine.registry,
            target: preview_targets.and_then(|targets| targets.first().copied()),
            scope,
        };
        let text = describe_card(&text_ctx, id);
        let costs = display_costs(&text_ctx, id);
        let base_label = def
            .map(|def| loc.entity_name(def.loc_key))
            .unwrap_or_else(|| card.def.as_str().to_string());
        let label = if card.upgraded {
            format!("{base_label}+")
        } else {
            base_label
        };
        let target_descriptions = preview_targets
            .map(|targets| {
                targets
                    .iter()
                    .copied()
                    .map(|target| {
                        let ctx = CardTextCtx {
                            state: &engine.state,
                            registry: &engine.registry,
                            target: Some(target),
                            scope,
                        };
                        describe_card(&ctx, id)
                            .lines
                            .iter()
                            .map(|line| loc.card_text_line(line))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Self {
            label,
            card_type: def
                .map(|def| loc.card_type(def.card_type).to_string())
                .unwrap_or_else(|| loc.ui("label.unknown").to_string()),
            cost: cost_label(costs, loc),
            costs,
            target: def.map(|def| def.target).unwrap_or(TargetType::None),
            keywords: text
                .keywords
                .iter()
                .map(|keyword| loc.card_keyword(*keyword).to_string())
                .collect(),
            description: text
                .lines
                .iter()
                .map(|line| loc.card_text_line(line))
                .collect(),
            target_descriptions,
        }
    }

    fn description_for_target(&self, target_index: Option<usize>) -> &[String] {
        target_index
            .and_then(|index| self.target_descriptions.get(index))
            .filter(|lines| !lines.is_empty())
            .map(Vec::as_slice)
            .unwrap_or(&self.description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    use crate::content::powers::{PowerDef, PowerRules};
    use crate::core::ids::{LocKey, PowerId, PowerInstanceId};
    use crate::core::query::{Decision, DecisionQuery, DecisionQueryKind, PreventReason};
    use crate::core::rules::prevent_by_current_listener;

    const TEST_RETAIN_AFTER_DEATH: PowerId = PowerId::new("TEST_RETAIN_AFTER_DEATH");

    #[test]
    fn three_nibbit_test_combat_builds_three_monsters() {
        let registry = StaticRegistry::standard();
        let engine = build_demo_engine(&registry, 7, 3);
        let loc = Localization::new(Language::Eng);
        let snapshot = CombatSnapshot::from_engine(&engine, &loc);

        assert_eq!(snapshot.monsters.len(), 3);
        assert!(snapshot.monsters.iter().all(|monster| monster.alive));
        assert!(snapshot
            .monsters
            .iter()
            .all(|monster| monster.label == "Nibbit"));
    }

    #[test]
    fn dead_non_revivable_monsters_are_hidden_from_snapshot() {
        let registry = StaticRegistry::standard();
        let mut engine = build_demo_engine(&registry, 7, 3);
        let first_monster = engine.state.combat().unwrap().monster_ids()[0];
        engine.state.mark_dead(first_monster).unwrap();
        let loc = Localization::new(Language::Eng);

        let snapshot = CombatSnapshot::from_engine(&engine, &loc);

        assert_eq!(snapshot.monsters.len(), 2);
        assert!(snapshot.monsters.iter().all(|monster| monster.alive));
    }

    #[test]
    fn dead_monsters_that_prevent_removal_stay_in_snapshot() {
        let mut registry = StaticRegistry::standard();
        registry.powers.register(retain_after_death_def());
        let mut engine = build_demo_engine(&registry, 7, 2);
        let first_monster = engine.state.combat().unwrap().monster_ids()[0];
        engine
            .state
            .apply_power(first_monster, TEST_RETAIN_AFTER_DEATH, Decimal::from(1))
            .unwrap();
        engine.state.mark_dead(first_monster).unwrap();
        let loc = Localization::new(Language::Eng);

        let snapshot = CombatSnapshot::from_engine(&engine, &loc);

        assert_eq!(snapshot.monsters.len(), 2);
        assert!(snapshot.monsters.iter().any(|monster| !monster.alive));
    }

    fn retain_after_death_def() -> PowerDef {
        PowerDef {
            id: TEST_RETAIN_AFTER_DEATH,
            loc_key: LocKey::new("power.test_retain_after_death"),
            rules: PowerRules {
                decide: Some(prevent_owner_removal_after_death),
                ..PowerRules::default()
            },
        }
    }

    fn prevent_owner_removal_after_death(
        ctx: &RuleCtx<'_>,
        power: PowerInstanceId,
        query: &DecisionQuery,
    ) -> Decision {
        let DecisionQueryKind::ShouldRemoveCreatureAfterDeath { creature } = query.kind else {
            return Decision::Allow;
        };
        let owns_power = ctx
            .state
            .combat()
            .and_then(|combat| combat.powers.get(&power))
            .map(|instance| instance.owner == creature)
            .unwrap_or(false);

        if owns_power {
            prevent_by_current_listener(ctx, PreventReason::KeepsCreatureInCombat)
        } else {
            Decision::Allow
        }
    }
}

fn card_label(engine: &Engine, id: CardInstanceId, loc: &Localization) -> String {
    engine
        .state
        .card(id)
        .and_then(|card| engine.registry.cards.get(card.def))
        .map(|def| loc.entity_name(def.loc_key))
        .unwrap_or_else(|| format!("{:?}", id))
}

fn cost_label(costs: CardCosts, loc: &Localization) -> String {
    let mut parts = Vec::new();
    if !matches!(costs.energy, CardCost::None) {
        parts.push(format!(
            "{}:{}",
            loc.ui("cost.energy"),
            single_cost_label(costs.energy, loc)
        ));
    }
    if !matches!(costs.stars, CardCost::None) {
        parts.push(format!(
            "{}:{}",
            loc.ui("cost.stars"),
            single_cost_label(costs.stars, loc)
        ));
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join("/")
    }
}

fn single_cost_label(cost: CardCost, loc: &Localization) -> String {
    loc.cost(cost)
}

fn monster_intent_label(engine: &Engine, monster: CreatureId, loc: &Localization) -> String {
    let Some(creature) = engine.state.creature(monster) else {
        return loc.ui("label.unknown").to_string();
    };
    let Some(model) = creature.model else {
        return loc.ui("label.unknown").to_string();
    };
    let Some(def) = engine.registry.monsters.get(model) else {
        return loc.ui("label.unknown").to_string();
    };
    let ctx = RuleCtx {
        state: &engine.state,
        registry: &engine.registry,
        listener: None,
    };
    loc.intent((def.intent)(&ctx, monster))
}
