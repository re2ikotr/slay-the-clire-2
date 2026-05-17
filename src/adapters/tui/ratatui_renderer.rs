use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as TerminalEvent, KeyCode, KeyEvent,
    KeyEventKind, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};

use crate::assets::{Language, Localization};
use crate::content::cards::TargetType;
use crate::core::state::{CardCost, CardCosts};

use super::animation::{AnimationClock, VisualEffect};
use super::profile::TerminalProfile;
use super::symbols::{Symbols, UiSymbol};
use super::theme::{Theme, UiRole};
use super::{
    display_width, format_power_list, CombatDriver, CombatSnapshot, CombatUiAction, CreatureView,
    MAX_CARDS_IN_HAND,
};

const TICK_RATE: Duration = Duration::from_millis(120);
const MESSAGE_HISTORY_LIMIT: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Hand,
    Monsters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPanel {
    Player,
    Monsters,
    Messages,
    Hand,
    DrawPile,
    DiscardPile,
    ExhaustPile,
}

impl FocusPanel {
    fn from_key(key: char) -> Option<Self> {
        match key.to_ascii_lowercase() {
            'p' => Some(Self::Player),
            'm' => Some(Self::Monsters),
            'g' => Some(Self::Messages),
            'c' => Some(Self::Hand),
            'd' => Some(Self::DrawPile),
            's' => Some(Self::DiscardPile),
            'x' => Some(Self::ExhaustPile),
            _ => None,
        }
    }

    fn key(self) -> char {
        match self {
            Self::Player => 'p',
            Self::Monsters => 'm',
            Self::Messages => 'g',
            Self::Hand => 'c',
            Self::DrawPile => 'd',
            Self::DiscardPile => 's',
            Self::ExhaustPile => 'x',
        }
    }

    fn title(self, loc: Localization) -> &'static str {
        match self {
            Self::Player => loc.ui("label.player"),
            Self::Monsters => loc.ui("label.monsters"),
            Self::Messages => loc.ui("label.messages"),
            Self::Hand => loc.ui("label.hand"),
            Self::DrawPile => loc.ui("label.draw"),
            Self::DiscardPile => loc.ui("label.discard"),
            Self::ExhaustPile => loc.ui("label.exhaust"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnemyTargetPreview {
    None,
    Single(usize),
    All,
}

impl EnemyTargetPreview {
    fn for_card(snapshot: &CombatSnapshot, hand_index: usize, selected_monster: usize) -> Self {
        let Some(card) = snapshot.hand.get(hand_index) else {
            return Self::None;
        };
        let has_alive_enemy = snapshot.monsters.iter().any(|monster| monster.alive);
        if !has_alive_enemy {
            return Self::None;
        }

        match card.target {
            TargetType::Enemy | TargetType::AnyCreature => {
                selected_display_monster_index(snapshot, selected_monster)
                    .map(Self::Single)
                    .unwrap_or(Self::None)
            }
            TargetType::AllEnemies => Self::All,
            TargetType::None
            | TargetType::RandomEnemy
            | TargetType::SelfTarget
            | TargetType::AnyAlly => Self::None,
        }
    }

    fn highlights(self, index: usize, monster: &CreatureView) -> bool {
        match self {
            Self::None => false,
            Self::Single(target_index) => index == target_index && monster.alive,
            Self::All => monster.alive,
        }
    }

    fn focused_marker(self, index: usize) -> bool {
        matches!(self, Self::Single(target_index) if index == target_index)
    }
}

pub(super) struct RatatuiCombatApp<D> {
    driver: D,
    loc: Localization,
    messages: VecDeque<String>,
    selected_card: usize,
    selected_monster: usize,
    focus: Focus,
    profile: TerminalProfile,
    theme: Theme,
    symbols: Symbols,
    clock: AnimationClock,
    show_help: bool,
    focused_panel: Option<FocusPanel>,
    focus_scroll: u16,
}

impl<D: CombatDriver> RatatuiCombatApp<D> {
    pub fn new(driver: D, language: Language, profile: TerminalProfile) -> Self {
        let loc = Localization::new(language);
        let messages = VecDeque::new();
        let mut app = Self {
            driver,
            loc,
            messages,
            selected_card: 0,
            selected_monster: 0,
            focus: Focus::Hand,
            profile,
            theme: Theme::new(profile),
            symbols: Symbols::new(profile),
            clock: AnimationClock::default(),
            show_help: false,
            focused_panel: None,
            focus_scroll: 0,
        };
        let snapshot = app.driver.snapshot(&app.loc);
        app.sync_focus_to_selected_card(&snapshot);
        app
    }

    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let run_result = self.run_terminal(&mut terminal);
        let restore_result = restore_terminal(&mut terminal);
        match (run_result, restore_result) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    fn run_terminal(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> io::Result<()> {
        let mut last_tick = Instant::now();
        loop {
            terminal.draw(|frame| self.render(frame))?;

            let timeout = TICK_RATE
                .checked_sub(last_tick.elapsed())
                .unwrap_or_default();
            if event::poll(timeout)? {
                match event::read()? {
                    TerminalEvent::Key(key) => {
                        if self.handle_key(key) {
                            break;
                        }
                    }
                    TerminalEvent::Mouse(mouse) => self.handle_mouse(mouse),
                    _ => {}
                }
            }

            if last_tick.elapsed() >= TICK_RATE {
                self.clock.advance();
                last_tick = Instant::now();
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        if self.focused_panel.is_some() {
            return self.handle_focused_key(key);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('?') | KeyCode::Char('h') => self.show_help = !self.show_help,
            KeyCode::Char('e') => self.submit(CombatUiAction::EndTurn),
            KeyCode::Char('r') => self.submit(CombatUiAction::Restart),
            KeyCode::Char('l') => self.toggle_language(),
            KeyCode::Char(value) => {
                if let Some(panel) = FocusPanel::from_key(value) {
                    self.toggle_focused_panel(panel);
                } else if value.is_ascii_digit() && value != '0' {
                    let index = value as usize - '1' as usize;
                    self.select_card(index);
                }
            }
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Left => self.select_previous_card(),
            KeyCode::Right => self.select_next_card(),
            KeyCode::Up => self.select_previous_monster(),
            KeyCode::Down => self.select_next_monster(),
            KeyCode::Enter => self.play_selected_card(),
            _ => {}
        }
        false
    }

    fn handle_focused_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Esc => self.close_focused_panel(),
            KeyCode::Char(value) => {
                if let Some(panel) = FocusPanel::from_key(value) {
                    self.toggle_focused_panel(panel);
                }
            }
            KeyCode::Up => self.scroll_focused_panel_up(1),
            KeyCode::Down => self.scroll_focused_panel_down(1),
            KeyCode::PageUp => self.scroll_focused_panel_up(8),
            KeyCode::PageDown => self.scroll_focused_panel_down(8),
            KeyCode::Home => self.focus_scroll = 0,
            KeyCode::End => self.focus_scroll = u16::MAX,
            _ => {}
        }
        false
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.focused_panel.is_none() {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => self.scroll_focused_panel_up(3),
            MouseEventKind::ScrollDown => self.scroll_focused_panel_down(3),
            _ => {}
        }
    }

    fn toggle_focused_panel(&mut self, panel: FocusPanel) {
        if self.focused_panel == Some(panel) {
            self.close_focused_panel();
        } else {
            self.focused_panel = Some(panel);
            self.focus_scroll = 0;
        }
    }

    fn close_focused_panel(&mut self) {
        self.focused_panel = None;
        self.focus_scroll = 0;
    }

    fn scroll_focused_panel_up(&mut self, amount: u16) {
        self.focus_scroll = self.focus_scroll.saturating_sub(amount);
    }

    fn scroll_focused_panel_down(&mut self, amount: u16) {
        self.focus_scroll = self.focus_scroll.saturating_add(amount);
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Hand => Focus::Monsters,
            Focus::Monsters => Focus::Hand,
        };
    }

    fn toggle_language(&mut self) {
        let next = match self.loc.language() {
            Language::Eng => Language::Zhs,
            Language::Zhs => Language::Eng,
        };
        self.loc.set_language(next);
        self.push_message(self.loc.format_language_changed());
    }

    fn submit(&mut self, action: CombatUiAction) {
        let result = self.driver.submit(action, &self.loc);
        self.push_messages(result.messages);
        self.clamp_selection();
    }

    fn play_selected_card(&mut self) {
        self.play_hand_index(self.selected_card);
    }

    fn play_hand_index(&mut self, hand_index: usize) {
        let snapshot = self.driver.snapshot(&self.loc);
        let monster_index = selected_alive_monster_index(&snapshot, self.selected_monster);
        let result = self.driver.submit(
            CombatUiAction::PlayHandCard {
                hand_index,
                monster_index,
            },
            &self.loc,
        );
        self.push_messages(result.messages);
        self.clamp_selection();
    }

    fn select_card(&mut self, hand_index: usize) {
        let snapshot = self.driver.snapshot(&self.loc);
        if hand_index >= snapshot.hand.len() {
            self.push_message(self.loc.format_no_card_at_hand_index(hand_index + 1));
            return;
        }
        self.select_card_in_snapshot(hand_index, &snapshot);
    }

    fn select_previous_card(&mut self) {
        let snapshot = self.driver.snapshot(&self.loc);
        if snapshot.hand.is_empty() {
            self.selected_card = 0;
            self.focus = Focus::Hand;
            return;
        }
        self.select_card_in_snapshot(self.selected_card.saturating_sub(1), &snapshot);
    }

    fn select_next_card(&mut self) {
        let snapshot = self.driver.snapshot(&self.loc);
        if snapshot.hand.is_empty() {
            self.selected_card = 0;
            self.focus = Focus::Hand;
            return;
        }
        let max = snapshot.hand.len().saturating_sub(1);
        self.select_card_in_snapshot(self.selected_card.saturating_add(1).min(max), &snapshot);
    }

    fn select_card_in_snapshot(&mut self, hand_index: usize, snapshot: &CombatSnapshot) {
        self.selected_card = hand_index;
        self.sync_focus_to_selected_card(snapshot);
    }

    fn sync_focus_to_selected_card(&mut self, snapshot: &CombatSnapshot) {
        match EnemyTargetPreview::for_card(snapshot, self.selected_card, self.selected_monster) {
            EnemyTargetPreview::Single(monster_index) => {
                self.focus = Focus::Monsters;
                self.selected_monster = monster_index;
            }
            EnemyTargetPreview::None | EnemyTargetPreview::All => {
                self.focus = Focus::Hand;
            }
        }
    }

    fn select_previous_monster(&mut self) {
        self.selected_monster = self.selected_monster.saturating_sub(1);
        self.focus = Focus::Monsters;
    }

    fn select_next_monster(&mut self) {
        let snapshot = self.driver.snapshot(&self.loc);
        let max = snapshot.monsters.len().saturating_sub(1);
        self.selected_monster = self.selected_monster.saturating_add(1).min(max);
        self.focus = Focus::Monsters;
    }

    fn clamp_selection(&mut self) {
        let snapshot = self.driver.snapshot(&self.loc);
        self.clamp_to_snapshot(&snapshot);
        self.sync_focus_to_selected_card(&snapshot);
    }

    fn clamp_to_snapshot(&mut self, snapshot: &CombatSnapshot) {
        self.selected_card = self
            .selected_card
            .min(snapshot.hand.len().saturating_sub(1));
        self.selected_monster = self
            .selected_monster
            .min(snapshot.monsters.len().saturating_sub(1));
    }

    fn push_messages(&mut self, messages: Vec<String>) {
        for message in messages {
            self.push_message(message);
        }
    }

    fn push_message(&mut self, message: String) {
        if self.messages.len() == MESSAGE_HISTORY_LIMIT {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        let snapshot = self.driver.snapshot(&self.loc);
        self.clamp_to_snapshot(&snapshot);
        if let Some(panel) = self.focused_panel {
            self.render_focused_panel(frame, &snapshot, panel);
            return;
        }

        let root = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(13),
            Constraint::Length(3),
        ])
        .split(frame.size());
        let hand_area =
            Layout::vertical([Constraint::Length(10), Constraint::Length(3)]).split(root[2]);

        let body = Layout::horizontal([
            Constraint::Percentage(32),
            Constraint::Percentage(42),
            Constraint::Percentage(26),
        ])
        .split(root[1]);

        self.render_header(frame, root[0], &snapshot);
        self.render_player(frame, body[0], &snapshot);
        self.render_monsters(frame, body[1], &snapshot);
        self.render_messages(frame, body[2]);
        self.render_hand(frame, hand_area[0], &snapshot);
        self.render_piles(frame, hand_area[1], &snapshot);
        self.render_footer(frame, root[3]);
    }

    fn render_focused_panel(
        &mut self,
        frame: &mut Frame<'_>,
        snapshot: &CombatSnapshot,
        panel: FocusPanel,
    ) {
        let lines = self.focus_panel_lines(snapshot, panel);
        let title = format!(
            "{}  [{}] {}",
            panel.title(self.loc),
            panel.key(),
            self.loc.ui("label.close")
        );
        let block = self.panel(&title);
        let inner = block.inner(frame.size());
        let max_scroll = scroll_limit(lines.len(), inner.height);
        self.focus_scroll = self.focus_scroll.min(max_scroll);

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .scroll((self.focus_scroll, 0)),
            frame.size(),
        );
    }

    fn focus_panel_lines(
        &self,
        snapshot: &CombatSnapshot,
        panel: FocusPanel,
    ) -> Vec<Line<'static>> {
        match panel {
            FocusPanel::Player => self.player_focus_lines(snapshot),
            FocusPanel::Monsters => self.monster_focus_lines(snapshot),
            FocusPanel::Messages => self.message_lines(),
            FocusPanel::Hand => self.hand_lines(snapshot),
            FocusPanel::DrawPile => self.pile_card_lines(&snapshot.draw_pile),
            FocusPanel::DiscardPile => self.pile_card_lines(&snapshot.discard_pile),
            FocusPanel::ExhaustPile => self.pile_card_lines(&snapshot.exhaust_pile),
        }
    }

    fn player_focus_lines(&self, snapshot: &CombatSnapshot) -> Vec<Line<'static>> {
        vec![
            Line::from(vec![
                Span::styled(
                    format!("{} ", self.symbols.get(UiSymbol::Heart)),
                    self.theme
                        .style(hp_role(snapshot.player.hp, snapshot.player.max_hp)),
                ),
                Span::styled(
                    format!("{}/{}", snapshot.player.hp, snapshot.player.max_hp),
                    self.theme.style(UiRole::Base),
                ),
                Span::raw("  "),
                Span::styled(
                    format!(
                        "{} {}",
                        self.symbols.get(UiSymbol::Block),
                        snapshot.player.block
                    ),
                    self.theme.style(UiRole::Base),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("{} ", self.symbols.get(UiSymbol::Energy)),
                    self.theme.style(UiRole::Energy),
                ),
                Span::styled(
                    format!("{}/{}", snapshot.energy, snapshot.max_energy),
                    self.theme.style(UiRole::Energy),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{} {}", self.symbols.get(UiSymbol::Star), snapshot.stars),
                    self.theme.style(UiRole::Prompt),
                ),
            ]),
            status_line(
                self.loc.ui("label.status"),
                self.loc.ui("label.none"),
                &snapshot.player.powers,
                self.theme,
                false,
            ),
        ]
    }

    fn monster_focus_lines(&self, snapshot: &CombatSnapshot) -> Vec<Line<'static>> {
        if snapshot.monsters.is_empty() {
            return vec![Line::styled(
                self.loc.ui("label.none"),
                self.theme.style(UiRole::Muted),
            )];
        }

        let target_preview =
            EnemyTargetPreview::for_card(snapshot, self.selected_card, self.selected_monster);
        let mut lines = Vec::new();
        for (index, monster) in snapshot.monsters.iter().enumerate() {
            let highlighted = target_preview.highlights(index, monster);
            let label_style = if highlighted {
                self.theme.style(UiRole::CardSelected)
            } else if monster.alive {
                self.theme.style(UiRole::Monster)
            } else {
                self.theme.style(UiRole::CardDisabled)
            };
            let dead_suffix = if monster.alive {
                String::new()
            } else {
                format!(
                    "  {} {}",
                    self.symbols.get(UiSymbol::Dead),
                    self.loc.ui("label.dead")
                )
            };
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "{}. {}  {}{}",
                    index + 1,
                    monster.label,
                    monster_hp_label(monster, self.symbols),
                    dead_suffix
                ),
                label_style,
            )]));
            lines.push(monster_intent_line(
                monster,
                self.theme,
                self.symbols,
                self.loc,
                highlighted,
            ));
            lines.push(status_line(
                self.loc.ui("label.status"),
                self.loc.ui("label.none"),
                &monster.powers,
                self.theme,
                highlighted,
            ));
            lines.push(Line::raw(""));
        }
        lines
    }

    fn message_lines(&self) -> Vec<Line<'static>> {
        if self.messages.is_empty() {
            return vec![Line::styled(
                self.loc.ui("label.empty"),
                self.theme.style(UiRole::Muted),
            )];
        }

        self.messages
            .iter()
            .map(|message| {
                let style = if message.contains(self.loc.ui("status.rejected"))
                    || message.contains(self.loc.ui("status.failed"))
                {
                    self.theme.style(UiRole::LogWarning)
                } else {
                    self.theme.style(UiRole::Log)
                };
                Line::styled(message.clone(), style)
            })
            .collect()
    }

    fn hand_lines(&self, snapshot: &CombatSnapshot) -> Vec<Line<'static>> {
        card_list_lines(
            &snapshot.hand,
            Some(self.selected_card),
            self.focus == Focus::Monsters,
            self.symbols,
            self.loc,
            self.theme,
            self.profile.animation,
            self.clock.tick(),
        )
    }

    fn pile_card_lines(&self, cards: &[super::CardView]) -> Vec<Line<'static>> {
        card_list_lines(
            cards,
            None,
            false,
            self.symbols,
            self.loc,
            self.theme,
            self.profile.animation,
            self.clock.tick(),
        )
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect, snapshot: &CombatSnapshot) {
        let title_style = VisualEffect::Pulse {
            first: UiRole::Title,
            second: UiRole::Prompt,
            period_ticks: 12,
        }
        .style(self.theme, self.profile.animation, self.clock.tick());
        let mut spans = vec![
            Span::styled(self.loc.ui("app.title"), title_style),
            Span::raw("  "),
            Span::styled(
                format!("{} {}", self.loc.ui("label.seed"), snapshot.seed),
                self.theme.style(UiRole::Muted),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} {}",
                    self.loc.ui("label.phase"),
                    self.loc.phase(snapshot.phase)
                ),
                self.theme.style(UiRole::Energy),
            ),
            Span::raw("  "),
            Span::styled(
                format!("lang {}", self.loc.language().code()),
                self.theme.style(UiRole::Muted),
            ),
        ];
        if area.width >= 100 {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                self.profile.label(),
                self.theme.style(UiRole::Panel),
            ));
        }
        let line = Line::from(spans);
        frame.render_widget(
            Paragraph::new(Text::from(vec![line])).block(self.panel("")),
            area,
        );
    }

    fn render_player(&self, frame: &mut Frame<'_>, area: Rect, snapshot: &CombatSnapshot) {
        let block = self.panel_with_role(self.loc.ui("label.player"), UiRole::Ironclad);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
        let hp_style = hp_role(snapshot.player.hp, snapshot.player.max_hp);
        let hp_label = if snapshot.player.block > 0 {
            format!(
                "{} {}/{}  {} {}",
                self.symbols.get(UiSymbol::Heart),
                snapshot.player.hp,
                snapshot.player.max_hp,
                self.symbols.get(UiSymbol::Block),
                snapshot.player.block
            )
        } else {
            format!(
                "{} {}/{}",
                self.symbols.get(UiSymbol::Heart),
                snapshot.player.hp,
                snapshot.player.max_hp
            )
        };
        frame.render_widget(
            Gauge::default()
                .ratio(ratio(snapshot.player.hp, snapshot.player.max_hp))
                .label(hp_label)
                .gauge_style(self.theme.style(hp_style)),
            chunks[0],
        );
        let mut resource_spans = vec![
            Span::styled(
                format!("{} ", self.symbols.get(UiSymbol::Energy)),
                self.theme.style(UiRole::Energy),
            ),
            Span::styled(
                format!("{}/{}", snapshot.energy, snapshot.max_energy),
                self.theme.style(UiRole::Energy),
            ),
        ];
        if snapshot.stars > 0 {
            resource_spans.extend([
                Span::raw("  "),
                Span::styled(
                    format!("{} {}", self.symbols.get(UiSymbol::Star), snapshot.stars),
                    self.theme.style(UiRole::Energy),
                ),
            ]);
        }
        frame.render_widget(Paragraph::new(Line::from(resource_spans)), chunks[1]);
        frame.render_widget(
            Paragraph::new(status_line(
                self.loc.ui("label.status"),
                self.loc.ui("label.none"),
                &snapshot.player.powers,
                self.theme,
                false,
            )),
            chunks[2],
        );
        self.render_panel_hint(frame, area, FocusPanel::Player);
    }

    fn render_monsters(&self, frame: &mut Frame<'_>, area: Rect, snapshot: &CombatSnapshot) {
        let block = self.panel(self.loc.ui("label.monsters"));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if snapshot.monsters.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::styled(
                    self.loc.ui("label.none"),
                    self.theme.style(UiRole::Muted),
                )),
                inner,
            );
            return;
        }

        let row_constraints = snapshot
            .monsters
            .iter()
            .map(|_| Constraint::Length(3))
            .collect::<Vec<_>>();
        let rows = Layout::vertical(row_constraints).split(inner);
        let target_preview =
            EnemyTargetPreview::for_card(snapshot, self.selected_card, self.selected_monster);
        for (index, monster) in snapshot.monsters.iter().enumerate() {
            let highlighted = target_preview.highlights(index, monster);
            let focused = target_preview.focused_marker(index) && self.focus == Focus::Monsters;
            if let Some(area) = rows.get(index) {
                self.render_monster(frame, *area, index, monster, highlighted, focused);
            }
        }
        self.render_panel_hint(frame, area, FocusPanel::Monsters);
    }

    fn render_monster(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        index: usize,
        monster: &CreatureView,
        highlighted: bool,
        focused: bool,
    ) {
        if area.height == 0 {
            return;
        }

        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
        let header = Layout::horizontal([
            Constraint::Length(monster_label_width(area.width)),
            Constraint::Min(10),
        ])
        .split(rows[0]);

        let label_style = if monster.alive {
            self.theme.style(UiRole::Monster)
        } else {
            self.theme.style(UiRole::CardDisabled)
        };
        let selected_style = self.theme.style(UiRole::CardSelected);
        let row_style = if highlighted {
            selected_style
        } else {
            label_style
        };
        let index_style = if highlighted {
            selected_style
        } else {
            self.theme.style(UiRole::Muted)
        };
        let marker_style = if focused {
            VisualEffect::Pulse {
                first: UiRole::Prompt,
                second: UiRole::Energy,
                period_ticks: 8,
            }
            .style(self.theme, self.profile.animation, self.clock.tick())
        } else {
            self.theme.style(UiRole::Muted)
        };
        let marker = if focused {
            self.symbols.get(UiSymbol::Prompt)
        } else {
            " "
        };
        let mut label_spans = vec![
            Span::styled(format!("{marker} "), marker_style),
            Span::styled(format!("{}. ", index + 1), index_style),
            Span::styled(monster.label.as_str(), row_style),
        ];
        if !monster.alive {
            label_spans.push(Span::styled(
                format!(
                    " {} {}",
                    self.symbols.get(UiSymbol::Dead),
                    self.loc.ui("label.dead")
                ),
                if highlighted {
                    selected_style
                } else {
                    self.theme.style(UiRole::Defeat)
                },
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(label_spans)), header[0]);

        let hp_style = hp_role(monster.hp, monster.max_hp);
        frame.render_widget(
            Gauge::default()
                .ratio(ratio(monster.hp, monster.max_hp))
                .label(monster_hp_label(monster, self.symbols))
                .gauge_style(self.theme.style(hp_style)),
            header[1],
        );

        frame.render_widget(
            Paragraph::new(monster_intent_line(
                monster,
                self.theme,
                self.symbols,
                self.loc,
                highlighted,
            )),
            rows[1],
        );
        frame.render_widget(
            Paragraph::new(status_line(
                self.loc.ui("label.status"),
                self.loc.ui("label.none"),
                &monster.powers,
                self.theme,
                highlighted,
            )),
            rows[2],
        );
    }

    fn render_messages(&self, frame: &mut Frame<'_>, area: Rect) {
        let lines = latest_visible_lines(self.message_lines(), area.height.saturating_sub(2));
        let items = lines.into_iter().map(ListItem::new).collect::<Vec<_>>();

        frame.render_widget(
            List::new(items).block(self.panel(self.loc.ui("label.messages"))),
            area,
        );
        self.render_panel_hint(frame, area, FocusPanel::Messages);
    }

    fn render_hand(&self, frame: &mut Frame<'_>, area: Rect, snapshot: &CombatSnapshot) {
        frame.render_widget(
            Paragraph::new(Text::from(self.hand_lines(snapshot)))
                .block(self.panel(&hand_title(self.loc, snapshot))),
            area,
        );
        self.render_panel_hint(frame, area, FocusPanel::Hand);
    }

    fn render_piles(&self, frame: &mut Frame<'_>, area: Rect, snapshot: &CombatSnapshot) {
        let piles = Layout::horizontal([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);

        self.render_pile_box(
            frame,
            piles[0],
            self.loc.ui("label.draw"),
            UiSymbol::Draw,
            snapshot.draw_pile.len(),
            FocusPanel::DrawPile,
        );
        self.render_pile_box(
            frame,
            piles[1],
            self.loc.ui("label.discard"),
            UiSymbol::Discard,
            snapshot.discard_pile.len(),
            FocusPanel::DiscardPile,
        );
        self.render_pile_box(
            frame,
            piles[2],
            self.loc.ui("label.exhaust"),
            UiSymbol::Exhaust,
            snapshot.exhaust_pile.len(),
            FocusPanel::ExhaustPile,
        );
    }

    fn render_pile_box(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &str,
        symbol: UiSymbol,
        count: usize,
        panel: FocusPanel,
    ) {
        let line = Line::from(vec![
            Span::styled(
                format!("{} ", self.symbols.get(symbol)),
                self.theme.style(UiRole::Muted),
            ),
            Span::styled(count.to_string(), self.theme.style(UiRole::Base)),
        ]);
        frame.render_widget(Paragraph::new(line).block(self.panel(title)), area);
        self.render_panel_hint(frame, area, panel);
    }

    fn render_panel_hint(&self, frame: &mut Frame<'_>, area: Rect, panel: FocusPanel) {
        if area.width < 10 || area.height < 3 {
            return;
        }
        let hint = format!("[{}] {}", panel.key(), self.loc.ui("label.view"));
        let width = display_width(&hint).min(area.width.saturating_sub(2) as usize) as u16;
        let rect = Rect::new(
            area.x + area.width.saturating_sub(width + 1),
            area.y + area.height.saturating_sub(2),
            width,
            1,
        );
        frame.render_widget(
            Paragraph::new(Line::styled(hint, self.theme.style(UiRole::Panel))),
            rect,
        );
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let help_key = if self.show_help {
            "help.tui.full"
        } else {
            "help.tui.compact"
        };
        let line = Line::from(vec![
            Span::styled(
                format!("{} ", self.symbols.get(UiSymbol::Prompt)),
                self.theme.style(UiRole::Prompt),
            ),
            Span::styled(self.loc.ui(help_key), self.theme.style(UiRole::Muted)),
        ]);
        frame.render_widget(
            Paragraph::new(Text::from(vec![line])).block(self.panel("")),
            area,
        );
    }

    fn panel<'a>(&self, title: &'a str) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .title(Line::styled(title, self.theme.style(UiRole::Title)))
            .border_style(self.theme.style(UiRole::Panel))
    }

    fn panel_with_role<'a>(&self, title: &'a str, role: UiRole) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .title(Line::styled(title, self.theme.style(role)))
            .border_style(self.theme.style(role))
    }
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()
}

fn selected_alive_monster_index(
    snapshot: &CombatSnapshot,
    selected_monster: usize,
) -> Option<usize> {
    let selected = snapshot.monsters.get(selected_monster)?;
    if selected.alive {
        Some(
            snapshot.monsters[..selected_monster]
                .iter()
                .filter(|monster| monster.alive)
                .count(),
        )
    } else {
        snapshot
            .monsters
            .iter()
            .position(|monster| monster.alive)
            .map(|index| {
                snapshot.monsters[..index]
                    .iter()
                    .filter(|monster| monster.alive)
                    .count()
            })
    }
}

fn selected_display_monster_index(
    snapshot: &CombatSnapshot,
    selected_monster: usize,
) -> Option<usize> {
    match snapshot.monsters.get(selected_monster) {
        Some(monster) if monster.alive => Some(selected_monster),
        _ => snapshot.monsters.iter().position(|monster| monster.alive),
    }
}

fn hand_title(loc: Localization, snapshot: &CombatSnapshot) -> String {
    format!(
        "{} {}/{}",
        loc.ui("label.hand"),
        snapshot.hand.len(),
        MAX_CARDS_IN_HAND
    )
}

fn scroll_limit(line_count: usize, visible_height: u16) -> u16 {
    line_count
        .saturating_sub(visible_height as usize)
        .min(u16::MAX as usize) as u16
}

fn latest_visible_lines(lines: Vec<Line<'static>>, visible_height: u16) -> Vec<Line<'static>> {
    let visible_height = visible_height as usize;
    if visible_height == 0 || lines.len() <= visible_height {
        return lines;
    }
    let skip = lines.len() - visible_height;
    lines.into_iter().skip(skip).collect()
}

fn card_list_lines(
    cards: &[super::CardView],
    selected_card: Option<usize>,
    pulse_selected_marker: bool,
    symbols: Symbols,
    loc: Localization,
    theme: Theme,
    animation_enabled: bool,
    tick: u64,
) -> Vec<Line<'static>> {
    if cards.is_empty() {
        return vec![Line::styled(
            loc.ui("label.empty"),
            theme.style(UiRole::Muted),
        )];
    }

    cards
        .iter()
        .enumerate()
        .map(|(index, card)| {
            let selected = selected_card == Some(index);
            let marker = if selected {
                symbols.get(UiSymbol::Prompt)
            } else {
                " "
            };
            let marker_style = if selected && pulse_selected_marker {
                VisualEffect::Pulse {
                    first: UiRole::Prompt,
                    second: UiRole::Energy,
                    period_ticks: 8,
                }
                .style(theme, animation_enabled, tick)
            } else {
                theme.style(UiRole::Muted)
            };
            let line_style = if selected {
                theme.style(UiRole::CardSelected)
            } else {
                theme.style(UiRole::CardPlayable)
            };
            let mut spans = vec![
                Span::styled(format!("{marker} "), marker_style),
                Span::styled(format!("{:>2}. ", index + 1), line_style),
            ];
            spans.extend(card_cost_spans(card.costs, symbols, loc, theme));
            spans.extend([
                Span::styled(card.label.clone(), line_style),
                Span::raw("  "),
                Span::styled(card.card_type.clone(), line_style),
            ]);
            Line::from(spans)
        })
        .collect()
}

fn monster_label_width(area_width: u16) -> u16 {
    area_width.saturating_sub(16).clamp(10, 24)
}

fn monster_hp_label(monster: &CreatureView, symbols: Symbols) -> String {
    if monster.block > 0 {
        format!(
            "{} {}/{}  {} {}",
            symbols.get(UiSymbol::Heart),
            monster.hp,
            monster.max_hp,
            symbols.get(UiSymbol::Block),
            monster.block
        )
    } else {
        format!(
            "{} {}/{}",
            symbols.get(UiSymbol::Heart),
            monster.hp,
            monster.max_hp
        )
    }
}

fn card_cost_spans(
    costs: CardCosts,
    symbols: Symbols,
    loc: Localization,
    theme: Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    append_resource_cost(
        &mut spans,
        costs.energy,
        symbols.get(UiSymbol::Energy),
        theme.style(UiRole::Energy),
        loc,
        false,
    );
    append_resource_cost(
        &mut spans,
        costs.stars,
        symbols.get(UiSymbol::Star),
        theme.style(UiRole::Prompt),
        loc,
        true,
    );
    if spans.is_empty() {
        spans.push(Span::styled("-  ", theme.style(UiRole::Muted)));
    } else {
        spans.push(Span::raw("  "));
    }
    spans
}

fn append_resource_cost(
    spans: &mut Vec<Span<'static>>,
    cost: CardCost,
    symbol: &'static str,
    style: Style,
    loc: Localization,
    hide_zero: bool,
) {
    if matches!(cost, CardCost::None) || matches!(cost, CardCost::Fixed(0) if hide_zero) {
        return;
    }
    if !spans.is_empty() {
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(format!("{symbol} "), style));
    spans.push(Span::styled(loc.cost(cost), style));
}

fn monster_intent_line(
    monster: &CreatureView,
    theme: Theme,
    symbols: Symbols,
    loc: Localization,
    selected: bool,
) -> Line<'static> {
    let value_style = if selected {
        theme.style(UiRole::CardSelected)
    } else {
        theme.style(UiRole::LogWarning)
    };
    Line::from(vec![
        Span::raw("    "),
        Span::styled(
            format!("{} ", loc.ui("label.intent")),
            theme.style(UiRole::Muted),
        ),
        Span::styled(
            format!("{} {}", symbols.get(UiSymbol::Intent), monster.intent),
            value_style,
        ),
    ])
}

fn status_line(
    label: &str,
    none: &str,
    powers: &[super::PowerView],
    theme: Theme,
    selected: bool,
) -> Line<'static> {
    let value = if powers.is_empty() {
        none.to_string()
    } else {
        format_power_list(powers)
    };
    let value_style = if selected {
        theme.style(UiRole::CardSelected)
    } else if powers.is_empty() {
        theme.style(UiRole::Muted)
    } else {
        theme.style(UiRole::LogWarning)
    };
    Line::from(vec![
        Span::styled(format!("{label}: "), theme.style(UiRole::Muted)),
        Span::styled(value, value_style),
    ])
}

fn hp_role(hp: i32, max_hp: i32) -> UiRole {
    if max_hp > 0 && hp * 4 <= max_hp {
        UiRole::HpLow
    } else {
        UiRole::HpNormal
    }
}

fn ratio(value: i32, max: i32) -> f64 {
    if max <= 0 {
        0.0
    } else {
        (value.max(0) as f64 / max as f64).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::CardView;
    use super::*;
    use crate::core::state::CombatPhase;

    #[test]
    fn all_enemy_target_preview_highlights_all_alive_enemies() {
        let snapshot = snapshot_with_card_target(TargetType::AllEnemies);
        let preview = EnemyTargetPreview::for_card(&snapshot, 0, 1);

        assert_eq!(preview, EnemyTargetPreview::All);
        assert!(preview.highlights(0, &snapshot.monsters[0]));
        assert!(!preview.highlights(1, &snapshot.monsters[1]));
        assert!(preview.highlights(2, &snapshot.monsters[2]));
        assert!(!preview.focused_marker(0));
        assert!(!preview.focused_marker(2));
    }

    #[test]
    fn self_and_none_target_preview_do_not_highlight_enemies() {
        for target in [TargetType::SelfTarget, TargetType::None] {
            let snapshot = snapshot_with_card_target(target);
            let preview = EnemyTargetPreview::for_card(&snapshot, 0, 0);

            assert_eq!(preview, EnemyTargetPreview::None);
            assert!(snapshot
                .monsters
                .iter()
                .enumerate()
                .all(|(index, monster)| !preview.highlights(index, monster)));
        }
    }

    #[test]
    fn single_enemy_target_preview_uses_one_alive_enemy() {
        let snapshot = snapshot_with_card_target(TargetType::Enemy);
        let preview = EnemyTargetPreview::for_card(&snapshot, 0, 1);

        assert_eq!(preview, EnemyTargetPreview::Single(0));
        assert!(preview.highlights(0, &snapshot.monsters[0]));
        assert!(!preview.highlights(1, &snapshot.monsters[1]));
        assert!(!preview.highlights(2, &snapshot.monsters[2]));
        assert!(preview.focused_marker(0));
    }

    fn snapshot_with_card_target(target: TargetType) -> CombatSnapshot {
        CombatSnapshot {
            seed: 0,
            phase: CombatPhase::PlayerAction,
            player: creature("Player", true),
            energy: 3,
            max_energy: 3,
            stars: 0,
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            monsters: vec![
                creature("Nibbit A", true),
                creature("Nibbit B", false),
                creature("Nibbit C", true),
            ],
            hand: vec![card(target)],
        }
    }

    fn creature(label: &str, alive: bool) -> CreatureView {
        CreatureView {
            label: label.to_string(),
            hp: if alive { 10 } else { 0 },
            max_hp: 10,
            block: 0,
            intent: String::new(),
            powers: Vec::new(),
            alive,
        }
    }

    fn card(target: TargetType) -> CardView {
        CardView {
            label: "Test".to_string(),
            card_type: "Attack".to_string(),
            cost: "1".to_string(),
            costs: CardCosts::default(),
            target,
        }
    }
}
