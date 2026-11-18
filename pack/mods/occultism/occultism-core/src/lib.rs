//! Occultism's rules for the pack's port: pentacles (Modonomicon's dense multiblocks, as Occultism's rituals check
//! them), ritual recipes and their clock, and spirit fire. Translated from Occultism 1.224.6
//! (<https://github.com/klikli-dev/occultism>, MIT, Copyright 2020 klikli-dev; see `LICENSE-occultism`). Nothing here
//! depends on the engine: the world is read through the small traits each module takes.

pub mod ingredient;
pub mod pentacle;
pub mod ritual;
pub mod spirit_fire;

pub use ingredient::Ingredient;
pub use pentacle::{Pentacle, Rotation};
pub use ritual::RitualRecipe;
pub use spirit_fire::SpiritFireRecipe;
