//! AE2 addons on the port: the shared definitions registry ([`defs`]) and each
//! addon's rows, one module per addon behind its `ae2-<id>` feature
//! (`pack/pack.toml`). An addon's game behaviour, when it has any, lives in
//! `game/ae2x/<id>/`.
pub mod defs;
#[cfg(feature = "ae2-megacells")]
pub mod megacells;
#[cfg(feature = "ae2-merequester")]
pub mod merequester;

/// Builds the registry, which installs the addon machines' block ids: call
/// before any world or block entity loads.
pub fn install() {
    let _ = defs::get();
}
