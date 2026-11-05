//! AEInfinityBooster 2.1.0's rows in the definitions registry and its
//! settings (feature `ae2-aeinfinitybooster`). Derived from AEInfinityBooster
//! by Hexeption (`ModItems`, `AEInfinityBoosterConfig`,
//! `AccessPointCardFilter`, `MixinWirelessTerminalMenuHost`,
//! `MixinWirelessMenu`, `MixinWirelessBlockEntity`),
//! https://github.com/Hexeption/AEInfinityBooster tag `v2.1.0`, LGPL-3.0;
//! see `ae2/addons/aeinfinitybooster/`. The game's side (the access point's
//! chunk) is in `game/ae2x/aeinfinitybooster/`.
use super::defs::{AccessPointCard, Defs};
use std::sync::OnceLock;

pub const NAMESPACE: &str = "aeinfinitybooster";
/// `ModItems.INFINITY_CARD`: unlimited range in the access point's dimension.
pub const INFINITY_CARD: &str = "aeinfinitybooster:infinity_card";
/// `ModItems.DIMENSION_CARD`: unlimited range from any dimension.
pub const DIMENSION_CARD: &str = "aeinfinitybooster:dimension_card";

/// `aeinfinitybooster-common.toml`, `[general]` (`AEInfinityBoosterConfig`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// `dimensionCardDrain`, 100 (0 and up).
    pub dimension_card_drain: i64,
    /// `infinityCardDrain`, 50 (0 and up).
    pub infinity_card_drain: i64,
    /// `chunkLoadingEnabled`, true.
    pub chunk_loading: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self { dimension_card_drain: 100, infinity_card_drain: 50, chunk_loading: true }
    }
}
impl Config {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let Some(general) = value.get("general") else {
            return Ok(config);
        };
        let drain = |key: &str| general.get(key).and_then(toml::Value::as_integer).filter(|n| (0..=i64::from(i32::MAX)).contains(n));
        if let Some(n) = drain("dimensionCardDrain") {
            config.dimension_card_drain = n;
        }
        if let Some(n) = drain("infinityCardDrain") {
            config.infinity_card_drain = n;
        }
        if let Some(b) = general.get("chunkLoadingEnabled").and_then(toml::Value::as_bool) {
            config.chunk_loading = b;
        }
        Ok(config)
    }
    /// The game's `config/aeinfinitybooster-common.toml`, read once. All the
    /// Mods 10 ships none, so the mod's defaults hold there.
    pub fn get() -> Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/aeinfinitybooster-common.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid AEInfinityBooster config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

/// The two cards (`boostedDistance`): an active access point holding the
/// Infinity Range Booster gives a terminal in its dimension the signal of one
/// 16 blocks away (a squared distance of 256) however far it is; holding the
/// Dimension Card, the signal of one 32 blocks away (1024) to a terminal in
/// any dimension. Its menu shows an unlimited range and the configured drain
/// of either; both keep its chunk loaded when the config says so.
pub fn register(defs: &mut Defs) {
    let config = Config::get();
    defs.add_access_point_card(AccessPointCard {
        item: INFINITY_CARD.to_owned(),
        signal: 256.0,
        same_dimension: true,
        shown_drain: config.infinity_card_drain as f64,
        chunk_loading: config.chunk_loading,
    });
    defs.add_access_point_card(AccessPointCard {
        item: DIMENSION_CARD.to_owned(),
        signal: 1024.0,
        same_dimension: false,
        shown_drain: config.dimension_card_drain as f64,
        chunk_loading: config.chunk_loading,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_and_corrections() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        let config = Config::parse("[general]\ninfinityCardDrain = 7\ndimensionCardDrain = -1\nchunkLoadingEnabled = false\n").unwrap();
        assert_eq!(config, Config { dimension_card_drain: 100, infinity_card_drain: 7, chunk_loading: false });
        assert_eq!(Config::parse("[general]\ninfinityCardDrain = \"x\"\n").unwrap().infinity_card_drain, 50);
    }

    #[test]
    fn the_card_is_a_booster_slot_item_with_a_fixed_signal() {
        let defs = crate::ae2x::defs::get();
        assert!(defs.access_point_accepts(INFINITY_CARD) && defs.access_point_accepts("ae2:wireless_booster"));
        assert!(!defs.access_point_accepts("ae2:energy_card"));
        let card = defs.access_point_card(INFINITY_CARD).unwrap();
        assert_eq!((card.signal, card.same_dimension), (256.0, true));
        assert!(defs.access_point_card("ae2:wireless_booster").is_none());
        let card = defs.access_point_card(DIMENSION_CARD).unwrap();
        assert!(defs.access_point_accepts(DIMENSION_CARD));
        assert_eq!((card.signal, card.same_dimension, card.shown_drain, card.chunk_loading), (1024.0, false, 100.0, true));
    }
}
