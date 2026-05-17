use ratatui::style::{Color, Modifier, Style};

use super::profile::{ColorMode, TerminalProfile};

#[derive(Clone, Copy, Debug)]
pub(super) enum UiRole {
    Base,
    CardDisabled,
    CardPlayable,
    CardSelected,
    Defeat,
    Energy,
    HpLow,
    HpNormal,
    Ironclad,
    Log,
    LogWarning,
    Monster,
    Muted,
    Panel,
    Prompt,
    Title,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Theme {
    profile: TerminalProfile,
}

impl Theme {
    pub fn new(profile: TerminalProfile) -> Self {
        Self { profile }
    }

    pub fn style(self, role: UiRole) -> Style {
        match self.profile.color {
            ColorMode::None => self.mono_style(role),
            ColorMode::Ansi16 => self.ansi16_style(role),
            ColorMode::Ansi256 => self.ansi256_style(role),
            ColorMode::TrueColor => self.truecolor_style(role),
        }
    }

    fn mono_style(self, role: UiRole) -> Style {
        match role {
            UiRole::Title
            | UiRole::Energy
            | UiRole::HpLow
            | UiRole::Ironclad
            | UiRole::LogWarning => Style::default().add_modifier(Modifier::BOLD),
            UiRole::CardSelected => Style::default().add_modifier(Modifier::REVERSED),
            UiRole::CardDisabled | UiRole::Muted | UiRole::Panel => {
                Style::default().add_modifier(Modifier::DIM)
            }
            _ => Style::default(),
        }
    }

    fn ansi16_style(self, role: UiRole) -> Style {
        let style = match role {
            UiRole::Base | UiRole::Log => Style::default().fg(Color::White),
            UiRole::CardDisabled | UiRole::Muted | UiRole::Panel => {
                Style::default().fg(Color::DarkGray)
            }
            UiRole::CardPlayable => Style::default().fg(Color::White),
            UiRole::CardSelected => Style::default().fg(Color::Black).bg(Color::Yellow),
            UiRole::Defeat | UiRole::HpLow => Style::default().fg(Color::Red),
            UiRole::Energy | UiRole::LogWarning => Style::default().fg(Color::Yellow),
            UiRole::HpNormal | UiRole::Ironclad => Style::default().fg(Color::Red),
            UiRole::Monster => Style::default().fg(Color::Magenta),
            UiRole::Prompt | UiRole::Title => Style::default().fg(Color::Cyan),
        };
        self.with_emphasis(role, style)
    }

    fn ansi256_style(self, role: UiRole) -> Style {
        let style = match role {
            UiRole::Base | UiRole::Log => Style::default().fg(Color::Indexed(252)),
            UiRole::CardDisabled | UiRole::Muted | UiRole::Panel => {
                Style::default().fg(Color::Indexed(244))
            }
            UiRole::CardPlayable => Style::default().fg(Color::Indexed(230)),
            UiRole::CardSelected => Style::default()
                .fg(Color::Indexed(16))
                .bg(Color::Indexed(220)),
            UiRole::Defeat | UiRole::HpLow => Style::default().fg(Color::Indexed(203)),
            UiRole::Energy | UiRole::LogWarning => Style::default().fg(Color::Indexed(220)),
            UiRole::HpNormal => Style::default().fg(Color::Indexed(160)),
            UiRole::Ironclad => Style::default().fg(Color::Indexed(88)),
            UiRole::Monster => Style::default().fg(Color::Indexed(176)),
            UiRole::Prompt | UiRole::Title => Style::default().fg(Color::Indexed(81)),
        };
        self.with_emphasis(role, style)
    }

    fn truecolor_style(self, role: UiRole) -> Style {
        let style = match role {
            UiRole::Base | UiRole::Log => Style::default().fg(Color::Rgb(224, 225, 221)),
            UiRole::CardDisabled | UiRole::Muted | UiRole::Panel => {
                Style::default().fg(Color::Rgb(128, 134, 144))
            }
            UiRole::CardPlayable => Style::default().fg(Color::Rgb(246, 241, 213)),
            UiRole::CardSelected => Style::default()
                .fg(Color::Rgb(21, 26, 33))
                .bg(Color::Rgb(244, 196, 48)),
            UiRole::Defeat | UiRole::HpLow => Style::default().fg(Color::Rgb(235, 94, 85)),
            UiRole::Energy | UiRole::LogWarning => Style::default().fg(Color::Rgb(255, 209, 102)),
            UiRole::HpNormal => Style::default().fg(Color::Rgb(185, 52, 45)),
            UiRole::Ironclad => Style::default().fg(Color::Rgb(125, 35, 32)),
            UiRole::Monster => Style::default().fg(Color::Rgb(222, 139, 240)),
            UiRole::Prompt | UiRole::Title => Style::default().fg(Color::Rgb(100, 223, 223)),
        };
        self.with_emphasis(role, style)
    }

    fn with_emphasis(self, role: UiRole, style: Style) -> Style {
        let style = match role {
            UiRole::Title
            | UiRole::Energy
            | UiRole::HpLow
            | UiRole::Ironclad
            | UiRole::CardSelected
            | UiRole::LogWarning
            | UiRole::Defeat => style.add_modifier(Modifier::BOLD),
            UiRole::CardDisabled | UiRole::Muted | UiRole::Panel => {
                style.add_modifier(Modifier::DIM)
            }
            _ => style,
        };
        style
    }
}
