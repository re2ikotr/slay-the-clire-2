//! Core library for the Slay the Clire 2 logic rewrite.
//!
//! The crate is intentionally split into a pure logic kernel (`core`), static
//! content definitions (`content`), lookup tables (`registry`), and external
//! adapters. The first playable target is CLI/TUI, but game rules should remain
//! usable by tests and future simulation APIs without UI dependencies.

pub mod adapters;
pub mod content;
pub mod core;
pub mod registry;
