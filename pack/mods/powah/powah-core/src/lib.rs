//! The rules of the pack's Powah port, free of the engine so they can be tested
//! alone: the seven tiers, `config/powah.json5`, the Furnator's burn and push and
//! what its screen shows. Written from Powah 6.2.10's behaviour on a Java server,
//! its config and its manual. LGPL-3.0-or-later; see the bundle's README.md.
pub mod config;
pub mod furnator;
pub mod tier;
pub mod view;
