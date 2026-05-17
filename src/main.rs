fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("smoke") => slay_the_clire_2::adapters::cli::run(),
        _ => slay_the_clire_2::adapters::tui::run(),
    }
}
