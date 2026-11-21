//! The rules of the pack's Just Dire Things port, free of the engine so they can be tested alone: the goo blocks of
//! the four tiers and their spreading recipes, the redstone control every machine shares, the Generator T1 and the
//! server config they read. Translated from Just Dire Things 1.5.7 (Direwolf20-MC/JustDireThings, MIT); see the
//! bundle's README.md and LICENSE-justdirethings.
pub mod breaker;
pub mod config;
pub mod generator;
pub mod goo;
pub mod recipe;
pub mod redstone;
pub mod view;

/// `Direction.values()`: down, up, north, south, west, east; a side's index in every per-side array here.
pub const SIDES: usize = 6;

/// The names of the six sides, as block state properties write them (`facing=down`).
pub const SIDE_NAMES: [&str; SIDES] = ["down", "up", "north", "south", "west", "east"];
