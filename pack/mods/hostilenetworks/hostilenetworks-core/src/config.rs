//! `HostileConfig`: the numbers the machines run on. ATM10 8.4 ships no `hostilenetworks` config file, so these are
//! the mod's defaults; `load` reads a Placebo `.cfg` (`I:"Sim Chamber Power Cap"=2000000`) when a pack has one.
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub sim_power_cap: i32,
    pub fab_power_cap: i32,
    pub fab_power_cost: i32,
    /// 0 off (and no training mode), 1 yes, 2 only up to tier boundaries.
    pub sim_model_upgrade: i32,
    pub action_upgrades_model: bool,
    pub continuous_accuracy: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sim_power_cap: 2_000_000,
            fab_power_cap: 1_000_000,
            fab_power_cost: 256,
            sim_model_upgrade: 1,
            action_upgrades_model: true,
            continuous_accuracy: true,
        }
    }
}

impl Config {
    /// Reads `hostilenetworks.cfg` of `dir`; a missing file or entry keeps the default.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("hostilenetworks.cfg")).map_or_else(|_| Self::default(), |t| Self::parse(&t))
    }

    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        for line in text.lines().map(str::trim) {
            let Some((head, value)) = line.split_once('=') else { continue };
            let Some((kind, name)) = head.split_once(':') else { continue };
            let name = name.trim().trim_matches('"');
            let value = value.trim();
            let int = value.parse::<i32>().ok();
            let flag = value.parse::<bool>().ok();
            match (kind, name) {
                ("I", "Sim Chamber Power Cap") => c.sim_power_cap = int.map_or(c.sim_power_cap, |v| v.max(1)),
                ("I", "Loot Fab Power Cap") => c.fab_power_cap = int.map_or(c.fab_power_cap, |v| v.max(1)),
                ("I", "Loot Fab Power Cost") => c.fab_power_cost = int.map_or(c.fab_power_cost, |v| v.max(0)),
                ("I", "Sim Chamber Upgrades Model") => c.sim_model_upgrade = int.map_or(c.sim_model_upgrade, |v| v.clamp(0, 2)),
                ("B", "Action Upgrades Model" | "Killing Upgrades Model") => c.action_upgrades_model = flag.unwrap_or(c.action_upgrades_model),
                ("B", "Continuous Accuracy") => c.continuous_accuracy = flag.unwrap_or(c.continuous_accuracy),
                _ => {}
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_mods() {
        let c = Config::default();
        assert_eq!((c.sim_power_cap, c.fab_power_cap, c.fab_power_cost), (2_000_000, 1_000_000, 256));
        assert_eq!((c.sim_model_upgrade, c.action_upgrades_model, c.continuous_accuracy), (1, true, true));
    }

    #[test]
    fn a_cfg_overrides_what_it_names() {
        let c = Config::parse("I:\"Loot Fab Power Cost\"=10\nI:\"Sim Chamber Upgrades Model\"=2\nB:\"Continuous Accuracy\"=false\n");
        assert_eq!((c.fab_power_cost, c.sim_model_upgrade, c.continuous_accuracy, c.sim_power_cap), (10, 2, false, 2_000_000));
    }
}
