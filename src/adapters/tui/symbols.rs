use super::profile::TerminalProfile;

#[derive(Clone, Copy, Debug)]
pub(super) enum UiSymbol {
    Block,
    Dead,
    Discard,
    Draw,
    Energy,
    Exhaust,
    Heart,
    Intent,
    Prompt,
    Star,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Symbols {
    unicode: bool,
}

impl Symbols {
    pub fn new(profile: TerminalProfile) -> Self {
        Self {
            unicode: profile.unicode,
        }
    }

    pub fn get(self, symbol: UiSymbol) -> &'static str {
        match (self.unicode, symbol) {
            (true, UiSymbol::Block) => "\u{25a3}",
            (false, UiSymbol::Block) => "BLK",
            (true, UiSymbol::Dead) => "\u{2715}",
            (false, UiSymbol::Dead) => "X",
            (true, UiSymbol::Discard) => "\u{21b4}",
            (false, UiSymbol::Discard) => "DIS",
            (true, UiSymbol::Draw) => "\u{21b1}",
            (false, UiSymbol::Draw) => "DRW",
            (true, UiSymbol::Energy) => "\u{25c8}",
            (false, UiSymbol::Energy) => "E",
            (true, UiSymbol::Exhaust) => "\u{25c7}",
            (false, UiSymbol::Exhaust) => "EXH",
            (true, UiSymbol::Heart) => "\u{2665}",
            (false, UiSymbol::Heart) => "HP",
            (true, UiSymbol::Intent) => "\u{25b8}",
            (false, UiSymbol::Intent) => ">",
            (true, UiSymbol::Prompt) => "\u{203a}",
            (false, UiSymbol::Prompt) => ">",
            (true, UiSymbol::Star) => "\u{2605}",
            (false, UiSymbol::Star) => "S",
        }
    }
}
