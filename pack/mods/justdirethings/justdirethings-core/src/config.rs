//! `config/justdirethings-server.toml` (`setup.Config`, its server spec): the goo's death and the Generator T1's
//! numbers. A missing file or key keeps the mod's default, which is also what ATM10 ships.
use std::{collections::HashMap, path::Path};

/// The settings the ported blocks read.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `goo.goo_can_die`: a goo may die when it finishes a conversion.
    pub goo_can_die: bool,
    /// `goo.goo_death_chance` (0 to 1): the chance it does.
    pub goo_death_chance: f64,
    pub generator: GeneratorSettings,
}

/// `generator_t1.*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorSettings {
    /// FE made for each burn tick of fuel (coal burns 1,600 ticks).
    pub fe_per_fuel_tick: i32,
    /// How many times faster than a furnace the generator burns its fuel.
    pub burn_speed_multiplier: i32,
    /// The buffer.
    pub max_fe: i32,
    /// FE a tick it hands to each neighbour (it offers ten times this).
    pub fe_per_tick: i32,
}

impl Default for GeneratorSettings {
    fn default() -> Self {
        Self { fe_per_fuel_tick: 15, burn_speed_multiplier: 4, max_fe: 1_000_000, fe_per_tick: 1000 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self { goo_can_die: true, goo_death_chance: 0.1, generator: GeneratorSettings::default() }
    }
}

/// A value of the TOML subset the mod writes.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Bool(bool),
    Number(f64),
}

impl Config {
    /// Reads `[section]` headers and `key = value` lines (booleans and numbers); everything else is skipped.
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
            } else if let Some((key, value)) = line.split_once('=') {
                let value = value.split('#').next().unwrap_or("").trim();
                let value = match value {
                    "true" => Some(Value::Bool(true)),
                    "false" => Some(Value::Bool(false)),
                    v => v.parse::<f64>().ok().map(Value::Number),
                };
                if let Some(value) = value {
                    values.insert(format!("{section}.{}", key.trim()), value);
                }
            }
        }
        let d = Self::default();
        let int = |key: &str, default: i32, min: i32, max: i32| match values.get(key) {
            Some(Value::Number(n)) if n.fract() == 0.0 && *n >= f64::from(min) && *n <= f64::from(max) => *n as i32,
            _ => default,
        };
        let g = d.generator;
        Self {
            goo_can_die: match values.get("goo.goo_can_die") {
                Some(Value::Bool(b)) => *b,
                _ => d.goo_can_die,
            },
            goo_death_chance: match values.get("goo.goo_death_chance") {
                Some(Value::Number(n)) if (0.0..=1.0).contains(n) => *n,
                _ => d.goo_death_chance,
            },
            generator: GeneratorSettings {
                fe_per_fuel_tick: int("generator_t1.generator_t1_fe_per_fuel_tick", g.fe_per_fuel_tick, 1, i32::MAX),
                burn_speed_multiplier: int("generator_t1.generator_t1_burn_speed_multiplier", g.burn_speed_multiplier, 1, 1000),
                max_fe: int("generator_t1.generator_t1_max_fe", g.max_fe, 1, i32::MAX),
                fe_per_tick: int("generator_t1.generator_t1_fe_per_tick", g.fe_per_tick, 1, i32::MAX),
            },
        }
    }

    /// `<dir>/justdirethings-server.toml`, or the defaults when it can't be read.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("justdirethings-server.toml")).map_or_else(|_| Self::default(), |t| Self::parse(&t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_mods() {
        let c = Config::default();
        assert!(c.goo_can_die);
        assert_eq!(c.goo_death_chance, 0.1);
        assert_eq!(c.generator, GeneratorSettings { fe_per_fuel_tick: 15, burn_speed_multiplier: 4, max_fe: 1_000_000, fe_per_tick: 1000 });
    }

    #[test]
    fn the_file_overrides_within_the_specs_ranges() {
        let c = Config::parse(
            "#Goo settings\n[goo]\n\tgoo_can_die = false\n\tgoo_death_chance = 0.5\n[generator_t1]\n\tgenerator_t1_fe_per_fuel_tick = 20\n\tgenerator_t1_burn_speed_multiplier = 5000\n\tgenerator_t1_max_fe = 500\n",
        );
        assert!(!c.goo_can_die);
        assert_eq!(c.goo_death_chance, 0.5);
        assert_eq!(c.generator.fe_per_fuel_tick, 20);
        assert_eq!(c.generator.burn_speed_multiplier, 4, "out of 1..1000: the default, as the spec's correction does");
        assert_eq!(c.generator.max_fe, 500);
        assert_eq!(c.generator.fe_per_tick, 1000);
        assert_eq!(Config::parse("[goo]\ngoo_death_chance = 2.0").goo_death_chance, 0.1);
    }
}
