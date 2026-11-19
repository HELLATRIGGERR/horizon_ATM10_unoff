//! The rules of the pack's Iron Furnaces port, free of the engine so they can be tested alone: the thirteen
//! furnace tiers and `config/ironfurnaces-common.toml`, the cooking time and the fuel a burn is worth, the
//! furnace's timers, how many items a tier smelts at once, and the tier upgrades. Translated from Iron Furnaces
//! 4.3.2 (Qelifern/IronFurnacesNeoForge, Apache-2.0); see the bundle's README.md and LICENSE-ironfurnaces.
pub mod config;
pub mod furnace;
pub mod tier;
pub mod upgrade;
