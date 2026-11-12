//! The AE2 port's game above horizon-client (docs/ae2-crates.md, platform layer): what
//! horizon-client's extension point (`game::ae2_ext`) calls. AE2's own game code moves here from
//! horizon-client's `game/ae2_*` step by step; until then this crate holds the extension it
//! installs, which every `Game` makes its own of.
#![forbid(unsafe_code)]
use horizon_client::game::ae2_ext::{self, Ae2Ext};

/// The AE2 of one game.
struct Ae2;

impl Ae2Ext for Ae2 {}

fn make() -> Box<dyn Ae2Ext> {
    Box::new(Ae2)
}

/// Installs AE2 into the game. The program calls it once, before the game starts.
pub fn install() {
    ae2_ext::install(make);
}
