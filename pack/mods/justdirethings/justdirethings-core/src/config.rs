//! `config/justdirethings-server.toml` (`setup.Config`, its server spec): the goo's death, the Generator T1's
//! numbers, where time crystals grow and the Paradox Machine's costs. A missing file or key keeps the mod's default, which is also what ATM10 ships.
use std::{collections::HashMap, path::Path};

/// The settings the ported blocks read.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `goo.goo_can_die`: a goo may die when it finishes a conversion.
    pub goo_can_die: bool,
    /// `goo.goo_death_chance` (0 to 1): the chance it does.
    pub goo_death_chance: f64,
    pub generator: GeneratorSettings,
    pub crystal: CrystalSettings,
    pub paradox: ParadoxSettings,
}

/// `time_crystal.*`: whether the three stage lists replace the normal rules (stage 1 outside the Nether and the End,
/// 2 in the Nether, 3 in the End), and the lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CrystalSettings {
    pub custom_dimensions: bool,
    pub stage1: Vec<String>,
    pub stage2: Vec<String>,
    pub stage3: Vec<String>,
}

/// `paradox_machine.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParadoxSettings {
    /// Time fluid the tank holds (mB).
    pub fluid_capacity: i32,
    /// Forge Energy the buffer holds.
    pub rf_capacity: i32,
    pub rf_per_block: i32,
    pub rf_per_entity: i32,
    pub fluid_per_block: i32,
    pub fluid_per_entity: i32,
    /// Paradox energy a restored block and a restored entity add.
    pub energy_per_block: f64,
    pub energy_per_entity: f64,
    /// The paradox energy at which a paradox appears.
    pub energy_max: f64,
    /// `paradox_restricted_mobs`: copy only a short list of a mob's fields, instead of all but its items.
    pub restricted_mobs: bool,
}

impl Default for ParadoxSettings {
    fn default() -> Self {
        Self {
            fluid_capacity: 16000,
            rf_capacity: 10_000_000,
            rf_per_block: 250_000,
            rf_per_entity: 250_000,
            fluid_per_block: 50,
            fluid_per_entity: 50,
            energy_per_block: 0.25,
            energy_per_entity: 0.25,
            energy_max: 100.0,
            restricted_mobs: false,
        }
    }
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
        Self {
            goo_can_die: true,
            goo_death_chance: 0.1,
            generator: GeneratorSettings::default(),
            crystal: CrystalSettings::default(),
            paradox: ParadoxSettings::default(),
        }
    }
}

/// A value of the TOML subset the mod writes.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Bool(bool),
    Number(f64),
    /// `["a", "b"]` on one line.
    List(Vec<String>),
}

/// `["minecraft:overworld", "a:b"]`: the quoted strings of a one-line list.
fn list(value: &str) -> Option<Vec<String>> {
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.split(',').map(|s| s.trim().trim_matches('"').to_owned()).filter(|s| !s.is_empty()).collect())
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
                    v if v.starts_with('[') => list(v).map(Value::List),
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
        let p = d.paradox;
        let boolean = |key: &str, default: bool| match values.get(key) {
            Some(Value::Bool(b)) => *b,
            _ => default,
        };
        let float = |key: &str, default: f64| match values.get(key) {
            Some(Value::Number(n)) if *n >= 0.0 => *n,
            _ => default,
        };
        let strings = |key: &str| match values.get(key) {
            Some(Value::List(l)) => l.clone(),
            _ => Vec::new(),
        };
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
            crystal: CrystalSettings {
                custom_dimensions: boolean("time_crystal.time_crystal_custom_dimensions", false),
                stage1: strings("time_crystal.time_crystal_stage_1_dims"),
                stage2: strings("time_crystal.time_crystal_stage_2_dims"),
                stage3: strings("time_crystal.time_crystal_stage_3_dims"),
            },
            paradox: ParadoxSettings {
                fluid_capacity: int("paradox_machine.paradox_fluid_capacity", p.fluid_capacity, 1, i32::MAX),
                rf_capacity: int("paradox_machine.paradox_rf_capacity", p.rf_capacity, 1, i32::MAX),
                rf_per_block: int("paradox_machine.paradox_rf_per_block", p.rf_per_block, 0, i32::MAX),
                rf_per_entity: int("paradox_machine.paradox_rf_per_entity", p.rf_per_entity, 0, i32::MAX),
                fluid_per_block: int("paradox_machine.paradox_fluid_per_block", p.fluid_per_block, 0, i32::MAX),
                fluid_per_entity: int("paradox_machine.paradox_fluid_per_entity", p.fluid_per_entity, 0, i32::MAX),
                energy_per_block: float("paradox_machine.paradox_energy_per_block", p.energy_per_block),
                energy_per_entity: float("paradox_machine.paradox_energy_per_entity", p.energy_per_entity),
                energy_max: float("paradox_machine.paradox_energy_max", p.energy_max),
                restricted_mobs: boolean("paradox_machine.paradox_restricted_mobs", p.restricted_mobs),
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

    #[test]
    fn time_crystal_lists_and_paradox_costs_read() {
        let c = Config::parse(
            "[time_crystal]\n\ttime_crystal_custom_dimensions = true\n\ttime_crystal_stage_1_dims = [\"minecraft:overworld\", \"a:b\"]\n\ttime_crystal_stage_2_dims = []\n[paradox_machine]\n\tparadox_rf_per_block = 1000\n\tparadox_energy_max = 2.5\n\tparadox_restricted_mobs = true\n",
        );
        assert!(c.crystal.custom_dimensions);
        assert_eq!(c.crystal.stage1, vec!["minecraft:overworld".to_owned(), "a:b".to_owned()]);
        assert!(c.crystal.stage2.is_empty() && c.crystal.stage3.is_empty());
        assert_eq!(c.paradox.rf_per_block, 1000);
        assert_eq!(c.paradox.energy_max, 2.5);
        assert!(c.paradox.restricted_mobs);
        assert_eq!(c.paradox.fluid_capacity, 16000);
        assert_eq!(Config::default().paradox, ParadoxSettings::default());
    }
}
