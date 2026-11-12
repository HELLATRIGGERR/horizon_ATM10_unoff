//! The AE2 port's game above horizon-client (docs/ae2-crates.md, platform layer): what
//! horizon-client's extension point (`game::ae2_ext`) calls. AE2's own game code moves here from
//! horizon-client's `game/ae2_*` step by step; until then this crate holds the extension it
//! installs, which every `Game` makes its own of, and the addons' game behaviour in it.
#![forbid(unsafe_code)]
use horizon_client::game::ae2_ext::{self, Ae2Addon, Ae2Ext};
use std::sync::{Mutex, PoisonError};

/// The addon crates' game behaviour, in the order they were added; every game makes its own.
static ADDONS: Mutex<Vec<fn() -> Box<dyn Ae2Addon>>> = Mutex::new(Vec::new());

/// Adds an addon crate's game behaviour. The program calls it before the game starts.
pub fn add_addon(make: fn() -> Box<dyn Ae2Addon>) {
    ADDONS.lock().unwrap_or_else(PoisonError::into_inner).push(make);
}

/// The AE2 of one game.
struct Ae2 {
    addons: Vec<Box<dyn Ae2Addon>>,
}

impl Ae2Ext for Ae2 {
    fn addon(&mut self, id: &str) -> Option<&mut dyn Ae2Addon> {
        self.addons.iter_mut().find(|addon| addon.id() == id).map(|addon| addon.as_mut() as &mut dyn Ae2Addon)
    }
}

fn make() -> Box<dyn Ae2Ext> {
    let addons = ADDONS.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|make| make()).collect();
    Box::new(Ae2 { addons })
}

/// Installs AE2 into the game. The program calls it once, before the game starts.
pub fn install() {
    ae2_ext::install(make);
}
