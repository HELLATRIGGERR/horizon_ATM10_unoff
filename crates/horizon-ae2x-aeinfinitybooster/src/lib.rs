//! AEInfinityBooster 2.1.0 on the AE2 port (pack/pack.toml's `aeinfinitybooster`, feature
//! `ae2-aeinfinitybooster` of horizon-atm10): the Infinity and Dimension Cards of the Wireless
//! Access Point. A crate above horizon-client (docs/ae2-crates.md): its rows go into the definitions
//! registry, and its game behaviour into AE2's extension, both installed before the game starts.
#![forbid(unsafe_code)]
mod defs;
mod game;

/// Installs the addon: its rows, where the registry had them, and its game behaviour. The program
/// calls it once, before the game starts.
pub fn install() {
    install_rows();
    horizon_ae2::add_addon(game::make);
}

fn install_rows() {
    horizon_client::ae2x::defs::install_addon_rows(defs::NAMESPACE, defs::register);
}
