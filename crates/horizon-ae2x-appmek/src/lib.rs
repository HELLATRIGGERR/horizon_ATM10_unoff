//! Applied Mekanistics 1.6.3 on the AE2 port (pack/pack.toml's `appmek`, feature `ae2-appmek` of
//! horizon-atm10): Mekanism's chemicals as an AE2 key type, their storage cells and portable cells.
//! A crate above horizon-client (docs/ae2-crates.md, docs/appmek.md): its rows go into the
//! definitions registry, in their place after Applied Flux's, and its game behaviour (the buses'
//! chemical strategies) into AE2's extension, both before the game starts.
#![forbid(unsafe_code)]
mod defs;
mod game;

pub use defs::{KEY_TYPE, NAMESPACE, chemical_key, key_type};

/// Installs the addon's rows and its game behaviour. The program calls it once, before the game
/// starts.
pub fn install() {
    horizon_client::ae2x::defs::install_addon_rows(defs::NAMESPACE, defs::register);
    horizon_ae2::add_addon(game::make);
}
