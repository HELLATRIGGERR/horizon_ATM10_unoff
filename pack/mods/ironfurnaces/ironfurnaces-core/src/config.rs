//! `config/ironfurnaces-common.toml` (`ironfurnaces.Config`): each furnace's cooking time and, for the modded
//! ones, how many items one cooking finishes. A missing file or key keeps the mod's default, which is also what
//! ATM10 ships.
use crate::tier::Tier;
use std::{collections::HashMap, path::Path};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IronFurnacesConfig {
    /// `section.key` (`furnaces.iron_furnace.speed`) to its number.
    values: HashMap<String, i64>,
}

impl IronFurnacesConfig {
    /// The mod's defaults: nothing overridden.
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Reads the TOML subset the mod writes: `[a.b]` headers and `key = integer` lines.
    pub fn parse(text: &str) -> Self {
        let mut values = HashMap::new();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = header.trim().to_owned();
            } else if let Some((key, value)) = line.split_once('=')
                && let Ok(number) = value.split('#').next().unwrap_or("").trim().parse::<i64>()
            {
                values.insert(format!("{section}.{}", key.trim()), number);
            }
        }
        Self { values }
    }

    /// `<dir>/ironfurnaces-common.toml`, or the defaults when it can't be read.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("ironfurnaces-common.toml")).map_or_else(|_| Self::default(), |t| Self::parse(&t))
    }

    /// `Config.xFurnaceSpeed`, clamped to the mod's range (2..72000; the modded tiers 1..72000).
    pub fn speed(&self, tier: Tier) -> i32 {
        let min = if matches!(tier, Tier::Allthemodium | Tier::Vibranium | Tier::Unobtainium) { 1 } else { 2 };
        self.get(tier, "speed", tier.default_speed()).clamp(min, 72000)
    }

    /// How many items one cooking finishes (`smelt`): the modded tiers' `mult` (1..64), the Rainbow furnace's
    /// fixed 64, 1 for the rest.
    pub fn mult(&self, tier: Tier) -> i32 {
        match tier {
            Tier::Allthemodium | Tier::Vibranium | Tier::Unobtainium => self.get(tier, "mult", tier.default_mult()).clamp(1, 64),
            _ => tier.default_mult(),
        }
    }

    fn get(&self, tier: Tier, key: &str, default: i32) -> i32 {
        self.values
            .get(&format!("{}.{key}", tier.config_section()))
            .map_or(default, |v| (*v).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATM10: &str = "[furnaces]\n\t[furnaces.iron_furnace]\n\t\ttier = 0\n\t\tspeed = 160\n[furnaces.rainbow_furnace]\n\tspeed = 20\n[modded_furnaces.vibranium_furnace]\n\tspeed = 3\n\tmult = 32\n";

    #[test]
    fn defaults_are_the_mods() {
        let c = IronFurnacesConfig::default_config();
        let speeds: Vec<_> = Tier::ALL.iter().map(|t| c.speed(*t)).collect();
        assert_eq!(speeds, [180, 160, 140, 120, 80, 40, 40, 20, 5, 20, 5, 3, 1]);
        let mults: Vec<_> = Tier::ALL.iter().map(|t| c.mult(*t)).collect();
        assert_eq!(mults, [1, 1, 1, 1, 1, 1, 1, 1, 1, 64, 16, 32, 64]);
    }

    #[test]
    fn the_file_overrides_and_is_clamped() {
        let c = IronFurnacesConfig::parse(ATM10);
        assert_eq!(c.speed(Tier::Iron), 160);
        assert_eq!(c.mult(Tier::Vibranium), 32);
        let c = IronFurnacesConfig::parse(
            "[furnaces.iron_furnace]\nspeed = 1\n[furnaces.gold_furnace]\nspeed = 999999\n[modded_furnaces.unobtainium_furnace]\nspeed = 0\nmult = 900\n",
        );
        assert_eq!(c.speed(Tier::Iron), 2, "a furnace is never faster than 2 ticks");
        assert_eq!(c.speed(Tier::Gold), 72000);
        assert_eq!(c.speed(Tier::Unobtainium), 1);
        assert_eq!(c.mult(Tier::Unobtainium), 64);
    }
}
