use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyEvent, KeyEventKind};
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
    format_power_list, CombatDriver, CombatSnapshot, CombatUiAction, CreatureView,
    MAX_CARDS_IN_HAND, MAX_MESSAGES,
};

const TICK_RATE: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Hand,
    Monsters,
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
        };
        let snapshot = app.driver.snapshot(&app.loc);
        app.sync_focus_to_selected_card(&snapshot);
        app
    }

    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
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
                if let TerminalEvent::Key(key) = event::read()? {
                    if self.handle_key(key) {
                        break;
                    }
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

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('?') | KeyCode::Char('h') => self.show_help = !self.show_help,
            KeyCode::Char('e') => self.submit(CombatUiAction::EndTurn),
            KeyCode::Char('r') => self.submit(CombatUiAction::Restart),
            KeyCode::Char('l') => self.toggle_language(),
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Left => self.select_previous_card(),
            KeyCode::Right => self.select_next_card(),
            KeyCode::Up => self.select_previous_monster(),
            KeyCode::Down => self.select_next_monster(),
            KeyCode::Enter => self.play_selected_card(),
            KeyCode::Char(value) if value.is_ascii_digit() && value != '0' => {
                let index = value as usize - '1' as usize;
                self.select_card(index);
            }
            _ => {}
        }
        false
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
        if selected_card_targets_monster(snapshot, self.selected_card) {
            self.focus = Focus::Monsters;
            if let Some(monster_index) =
                selected_display_monster_index(snapshot, self.selected_monster)
            {
                self.selected_monster = monster_index;
            }
        } else {
            self.focus = Focus::Hand;
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
        if self.messages.len() == MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        let snapshot = self.driver.snapshot(&self.loc);
        self.clamp_to_snapshot(&snapshot);

        let root = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(10),
            Constraint::Length(3),
        ])
        .split(frame.size());

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
        self.render_hand(frame, root[2], &snapshot);
        self.render_footer(frame, root[3]);
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
            Constraint::Min(1),
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
        resource_spans.extend([
            Span::raw("  "),
            Span::styled(
                format!("{} {}", self.loc.ui("label.hand"), snapshot.hand.len()),
                self.theme.style(UiRole::Base),
            ),
            Span::styled(
                format!("/{}", MAX_CARDS_IN_HAND),
                self.theme.style(UiRole::Muted),
            ),
        ]);
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
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                pile_line(
                    self.symbols,
                    self.theme,
                    self.loc.ui("label.draw"),
                    UiSymbol::Draw,
                    snapshot.draw_count,
                ),
                pile_line(
                    self.symbols,
                    self.theme,
                    self.loc.ui("label.discard"),
                    UiSymbol::Discard,
                    snapshot.discard_count,
                ),
                pile_line(
                    self.symbols,
                    self.theme,
                    self.loc.ui("label.exhaust"),
                    UiSymbol::Exhaust,
                    snapshot.exhaust_count,
                ),
                pile_line(
                    self.symbols,
                    self.theme,
                    self.loc.ui("label.removed"),
                    UiSymbol::Removed,
                    snapshot.removed_count,
                ),
            ])),
            chunks[3],
        );
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
        for (index, monster) in snapshot.monsters.iter().enumerate() {
            let selected = index == self.selected_monster;
            if let Some(area) = rows.get(index) {
                self.render_monster(frame, *area, index, monster, selected);
            }
        }
    }

    fn render_monster(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        index: usize,
        monster: &CreatureView,
        selected: bool,
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
        let row_style = if selected {
            selected_style
        } else {
            label_style
        };
        let index_style = if selected {
            selected_style
        } else {
            self.theme.style(UiRole::Muted)
        };
        let marker_style = if selected && self.focus == Focus::Monsters {
            VisualEffect::Pulse {
                first: UiRole::Prompt,
                second: UiRole::Energy,
                period_ticks: 8,
            }
            .style(self.theme, self.profile.animation, self.clock.tick())
        } else {
            self.theme.style(UiRole::Muted)
        };
        let marker = if selected {
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
                if selected {
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
                selected,
            )),
            rows[1],
        );
        frame.render_widget(
            Paragraph::new(status_line(
                self.loc.ui("label.status"),
                self.loc.ui("label.none"),
                &monster.powers,
                self.theme,
                selected,
            )),
            rows[2],
        );
    }

    fn render_messages(&self, frame: &mut Frame<'_>, area: Rect) {
        let items = self
            .messages
            .iter()
            .map(|message| {
                let style = if message.contains(self.loc.ui("status.rejected"))
                    || message.contains(self.loc.ui("status.failed"))
                {
                    self.theme.style(UiRole::LogWarning)
                } else {
                    self.theme.style(UiRole::Log)
                };
                ListItem::new(Line::styled(message.as_str(), style))
            })
            .collect::<Vec<_>>();

        frame.render_widget(
            List::new(items).block(self.panel(self.loc.ui("label.messages"))),
            area,
        );
    }

    fn render_hand(&self, frame: &mut Frame<'_>, area: Rect, snapshot: &CombatSnapshot) {
        let mut lines = Vec::new();
        if snapshot.hand.is_empty() {
            lines.push(Line::styled(
                self.loc.ui("label.empty"),
                self.theme.style(UiRole::Muted),
            ));
        }

        for (index, card) in snapshot.hand.iter().enumerate() {
            let selected = index == self.selected_card;
            let marker = if selected {
                self.symbols.get(UiSymbol::Prompt)
            } else {
                " "
            };
            let marker_style = if selected && self.focus == Focus::Monsters {
                VisualEffect::Pulse {
                    first: UiRole::Prompt,
                    second: UiRole::Energy,
                    period_ticks: 8,
                }
                .style(self.theme, self.profile.animation, self.clock.tick())
            } else {
                self.theme.style(UiRole::Muted)
            };
            let line_style = if selected {
                self.theme.style(UiRole::CardSelected)
            } else {
                self.theme.style(UiRole::CardPlayable)
            };
            let mut spans = vec![
                Span::styled(format!("{marker} "), marker_style),
                Span::styled(format!("{:>2}. ", index + 1), line_style),
            ];
            spans.extend(card_cost_spans(
                card.costs,
                self.symbols,
                self.loc,
                self.theme,
            ));
            spans.extend([
                Span::styled(card.label.as_str(), line_style),
                Span::raw("  "),
                Span::styled(card.card_type.as_str(), line_style),
            ]);
            lines.push(Line::from(spans));
        }

        frame.render_widget(
            Paragraph::new(Text::from(lines)).block(self.panel(self.loc.ui("label.hand"))),
            area,
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
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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

fn selected_card_targets_monster(snapshot: &CombatSnapshot, hand_index: usize) -> bool {
    snapshot
        .hand
        .get(hand_index)
        .map(|card| card_targets_monster(card.target))
        .unwrap_or(false)
        && snapshot.monsters.iter().any(|monster| monster.alive)
}

fn card_targets_monster(target: TargetType) -> bool {
    matches!(target, TargetType::Enemy)
}

fn pile_line(
    symbols: Symbols,
    theme: Theme,
    label: &str,
    symbol: UiSymbol,
    count: usize,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{} ", symbols.get(symbol)),
            theme.style(UiRole::Muted),
        ),
        Span::styled(format!("{label}: {count}"), theme.style(UiRole::Base)),
    ])
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

fn monster_intent_line<'a>(
    monster: &'a CreatureView,
    theme: Theme,
    symbols: Symbols,
    loc: Localization,
    selected: bool,
) -> Line<'a> {
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
