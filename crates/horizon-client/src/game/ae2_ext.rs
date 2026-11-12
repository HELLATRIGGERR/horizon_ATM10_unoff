//! The extension point of the fork's AE2 crates above horizon-client (docs/ae2-crates.md): what
//! the game calls on them ([`Ae2Ext`]) and what they may read and do in it ([`Ae2Host`],
//! implemented for `Game` in `ae2_host.rs`). It names no type from above (docs/layers.md). The
//! program that has them (horizon-atm10) installs a factory before the game starts, and every
//! `Game` makes its own extension with it.
//!
//! Calls across are `dyn` only, both ways: no generic function and no `impl Trait` parameter on
//! either side, so none of the crates' code is compiled into horizon-client.
use std::sync::OnceLock;

/// What the AE2 crates above may read and do in the game.
pub trait Ae2Host {}

/// What the game calls on the AE2 crates above: one per `Game`, with their state.
pub trait Ae2Ext {}

/// Makes one game's extension.
pub type Factory = fn() -> Box<dyn Ae2Ext>;

static FACTORY: OnceLock<Factory> = OnceLock::new();

/// Installs the extension's factory. The program calls it once, before the game starts; the first
/// install holds.
pub fn install(factory: Factory) {
    let _ = FACTORY.set(factory);
}
