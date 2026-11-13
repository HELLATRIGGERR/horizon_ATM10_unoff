//! The upgrade kits of ExtendedAE and Advanced AE on the AE2 port: the item that, used on a device,
//! puts another in its place with all it holds. A crate above horizon-client (docs/ae2-crates.md),
//! shared by the two addons: the kit rows and what a kit makes of a device are the lib's
//! `ae2x::kits`; the use is here, an addon of AE2's extension that the addons call by
//! `Ae2Host::call_addon("kits", USE, ...)` while they are inside horizon-client, and by [`kit_use`]
//! once they sit above it too.
#![forbid(unsafe_code)]
mod game;

pub use game::{USE, UseArgs, kit_use};

/// The id the addons call the kits by.
pub const ID: &str = "kits";

/// Installs the kits' use into AE2's extension. The program calls it once, before the game starts.
pub fn install() {
    horizon_ae2::add_addon(game::make);
}
