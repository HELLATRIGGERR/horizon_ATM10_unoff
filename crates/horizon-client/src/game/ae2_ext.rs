//! The extension point of the fork's AE2 crates above horizon-client (docs/ae2-crates.md): what
//! the game calls on them ([`Ae2Ext`], [`Ae2Addon`]) and what they may read and do in it
//! ([`Ae2Host`], implemented for `Game` in `ae2_host.rs`). It names no type from above
//! (docs/layers.md). The program that has them (horizon-atm10) installs a factory before the game
//! starts, and every `Game` makes its own extension with it.
//!
//! Calls across are `dyn` only, both ways: no generic function and no `impl Trait` parameter on
//! either side, so none of the crates' code is compiled into horizon-client.
use horizon_client::{streaming::Streaming, world::World};
use std::sync::OnceLock;

/// What the AE2 crates above may read and do in the game.
pub trait Ae2Host {
    /// Whether this game is a client of a server (`Game::remote`), whose world it only shows.
    fn is_remote(&self) -> bool;
    /// The dimension of the level the game holds (`minecraft:overworld`...).
    fn dimension(&self) -> &str;
    /// That level's world.
    fn world(&self) -> &World;
    /// The chunk loader, whose `forced` chunks stay loaded.
    fn streaming(&mut self) -> &mut Streaming;
}

/// One addon's game behaviour (a `horizon-ae2x-<id>` crate), with what it keeps while the game
/// runs. The game calls each method where it called the addon's code before it moved.
pub trait Ae2Addon {
    /// The addon's id (pack/pack.toml's).
    fn id(&self) -> &'static str;
    /// A running game tick, in the addon's place in `tick_ae2x`.
    fn tick(&mut self, host: &mut dyn Ae2Host);
}

/// What the game calls on the AE2 crates above: one per `Game`, with their state.
pub trait Ae2Ext {
    /// The addon `id`, when it is built in.
    fn addon(&mut self, id: &str) -> Option<&mut dyn Ae2Addon>;
}

/// Makes one game's extension.
pub type Factory = fn() -> Box<dyn Ae2Ext>;

static FACTORY: OnceLock<Factory> = OnceLock::new();

/// Installs the extension's factory. The program calls it once, before the game starts; the first
/// install holds.
pub fn install(factory: Factory) {
    let _ = FACTORY.set(factory);
}

/// A game's extension, made with the game; none when nothing is installed. A call takes it out of
/// the game, so that it can have the game as its host, and puts it back after.
pub(super) struct Slot(Option<Box<dyn Ae2Ext>>);

impl Default for Slot {
    fn default() -> Self {
        Self(FACTORY.get().map(|make| make()))
    }
}

impl Slot {
    pub(super) fn take(&mut self) -> Option<Box<dyn Ae2Ext>> {
        self.0.take()
    }

    pub(super) fn put_back(&mut self, ext: Box<dyn Ae2Ext>) {
        self.0 = Some(ext);
    }
}
