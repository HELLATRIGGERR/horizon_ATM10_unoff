//! FTB Quests' data and rules, for the pack's quest book: its SNBT files, the
//! book they make, its translations, a player's progress, and the graph's
//! geometry. Nothing here touches the game, so all of it is tested on its own.
//! No quest data lives in this crate: it reads the player's own instance.
pub mod book;
pub mod filter;
pub mod graph;
pub mod lang;
pub mod names;
pub mod progress;
pub mod snbt;
pub mod text;
pub mod wire;
