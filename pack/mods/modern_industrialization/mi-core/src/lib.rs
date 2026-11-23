//! Modern Industrialization's machine rules, translated to Rust from MI 2.5.11's own source
//! (<https://github.com/AztechMC/Modern-Industrialization>, tag 2.5.11). Nothing here depends on the engine:
//! the bundle's runtime modules feed it the world's recipes, the block entity's slots and energy.
//!
//! Modern Industrialization is Copyright (c) 2020 Azercoco & Technici4n, MIT licence; see
//! `LICENSE-modern-industrialization` beside this crate.

pub mod crafter;
pub mod energy;
pub mod inventory;
pub mod machines;
pub mod multiblock;
pub mod recipe;
pub mod view;
