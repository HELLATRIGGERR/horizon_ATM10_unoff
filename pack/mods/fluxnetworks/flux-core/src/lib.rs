//! The rules of Flux Networks 8.0.0 as the pack's port runs them, with no game
//! in them: the transfer cycle of `ServerFluxNetwork`, the handlers of plugs and
//! points, the saved networks, the settings ATM10 ships and the chest menu that
//! stands in for the mod's GUI. `pack/mods/fluxnetworks/runtime/flux_world.rs`
//! ties them to the game.
//!
//! Ported from Flux Networks' source, MIT, Copyright (c) 2017-2021 SonarSonic,
//! BloCamLimb (full notice in `pack/mods/fluxnetworks/README.md`).
pub mod config;
pub mod menu;
pub mod networks;
pub mod transfer;
pub mod world;
