use ratatui::style::Style;

use super::theme::{Theme, UiRole};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AnimationClock {
    tick: u64,
}

impl AnimationClock {
    pub fn tick(self) -> u64 {
        self.tick
    }

    pub fn advance(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum VisualEffect {
    Pulse {
        first: UiRole,
        second: UiRole,
        period_ticks: u64,
    },
}

impl VisualEffect {
    pub fn style(self, theme: Theme, animation_enabled: bool, tick: u64) -> Style {
        theme.style(self.role(animation_enabled, tick))
    }

    fn role(self, animation_enabled: bool, tick: u64) -> UiRole {
        match self {
            Self::Pulse {
                first,
                second,
                period_ticks,
            } if animation_enabled && period_ticks > 1 => {
                if tick % period_ticks < period_ticks / 2 {
                    first
                } else {
                    second
                }
            }
            Self::Pulse { first, .. } => first,
        }
    }
}
