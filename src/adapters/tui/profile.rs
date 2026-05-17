use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColorMode {
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TerminalProfile {
    pub color: ColorMode,
    pub unicode: bool,
    pub animation: bool,
    pub native_blink: bool,
    plain_renderer: bool,
}

impl TerminalProfile {
    pub fn detect(args: &[String]) -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let ci = std::env::var_os("CI").is_some();
        let stdout_is_tty = std::io::stdout().is_terminal();

        let mut profile = Self {
            color: if no_color {
                ColorMode::None
            } else {
                detect_color_mode(&term)
            },
            unicode: term != "dumb",
            animation: term != "dumb" && !ci,
            native_blink: false,
            plain_renderer: term == "dumb" || !stdout_is_tty,
        };

        for arg in args {
            match arg.as_str() {
                "--plain" => profile.plain_renderer = true,
                "--tui" | "--rich" => profile.plain_renderer = false,
                "--ascii" => profile.unicode = false,
                "--unicode" => profile.unicode = true,
                "--no-color" => profile.color = ColorMode::None,
                "--no-anim" => profile.animation = false,
                "--anim" => profile.animation = true,
                "--native-blink" => profile.native_blink = true,
                "--no-native-blink" => profile.native_blink = false,
                _ => {
                    if let Some(value) = arg.strip_prefix("--color=") {
                        if let Some(color) = parse_color_mode(value) {
                            profile.color = color;
                        }
                    }
                }
            }
        }

        if profile.plain_renderer {
            profile.color = ColorMode::None;
            profile.animation = false;
            profile.native_blink = false;
        }

        profile
    }

    pub fn use_plain_renderer(self) -> bool {
        self.plain_renderer
    }

    pub fn label(self) -> String {
        let color = match self.color {
            ColorMode::None => "mono",
            ColorMode::Ansi16 => "16-color",
            ColorMode::Ansi256 => "256-color",
            ColorMode::TrueColor => "truecolor",
        };
        let charset = if self.unicode { "unicode" } else { "ascii" };
        let animation = if self.animation { "anim" } else { "static" };
        let blink = if self.native_blink { "/blink" } else { "" };
        format!("{color}/{charset}/{animation}{blink}")
    }
}

fn detect_color_mode(term: &str) -> ColorMode {
    let colorterm = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(colorterm.as_str(), "truecolor" | "24bit") {
        return ColorMode::TrueColor;
    }
    if term.contains("256color") {
        return ColorMode::Ansi256;
    }
    if term.is_empty() || term == "dumb" {
        ColorMode::None
    } else {
        ColorMode::Ansi16
    }
}

fn parse_color_mode(value: &str) -> Option<ColorMode> {
    match value.to_ascii_lowercase().as_str() {
        "off" | "none" | "no" | "mono" => Some(ColorMode::None),
        "16" | "ansi16" | "basic" => Some(ColorMode::Ansi16),
        "256" | "ansi256" => Some(ColorMode::Ansi256),
        "true" | "truecolor" | "24bit" | "rgb" => Some(ColorMode::TrueColor),
        _ => None,
    }
}
