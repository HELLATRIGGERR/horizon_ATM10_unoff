//! The energy settings of Mekanism's config files, as ATM10 ships them in
//! `config/Mekanism/`: `general.toml` (`[energy_conversion]`,
//! `blockDeactivationDelay`) and `tiers.toml` (`[energy_cubes]`,
//! `[transmitters.energy]`), the unit screens show from `common.toml`
//! (`energyType`), and Mekanism Generators' `generators.toml`
//! (`[solar_generator]`, `[heat_generator]`) and `generator-storage.toml`. A
//! missing file, key or a value out of the mod's range keeps the default, as
//! NeoForge's config spec corrects it.
use crate::tier::{CableTier, CubeTier};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct EnergyConfig {
    /// `blacklistForge`: no Forge Energy in or out.
    pub blacklist_forge: bool,
    /// `feConversionRate`: Joules to the FE (FE * rate = J), 1e-4..=1e4.
    pub fe_conversion_rate: f64,
    /// `<tier>Capacity` and `<tier>Output` of each cube tier, in `CubeTier::ALL` order.
    pub cubes: [(u64, u64); 5],
    /// `<tier>Capacity` of each cable tier, in `CableTier::ALL` order.
    pub cables: [u64; 4],
    /// `energyType`: screens show Forge Energy (`FORGE_ENERGY`, the
    /// default) or Joules (`JOULES`).
    pub show_fe: bool,
    /// `blockDeactivationDelay`: ticks before a block that stopped working
    /// shows it (`active=false`), 0..=1200.
    pub deactivation_delay: u32,
    /// Mekanism Generators' settings.
    pub generators: GeneratorsConfig,
}

impl Default for EnergyConfig {
    fn default() -> Self {
        Self {
            blacklist_forge: false,
            fe_conversion_rate: 2.5,
            cubes: CubeTier::ALL.map(|t| (t.default_capacity(), t.default_output())),
            cables: CableTier::ALL.map(CableTier::default_capacity),
            show_fe: true,
            deactivation_delay: 60,
            generators: GeneratorsConfig::default(),
        }
    }
}

/// `generators.toml` and `generator-storage.toml` of Mekanism Generators
/// (`GeneratorsConfig`, `GeneratorsStorageConfig`), with Mekanism's defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorsConfig {
    /// `[solar_generator] solarGeneration`: Joules a tick at peak, 1 or more.
    pub solar_generation: u64,
    /// `[heat_generator] heatGeneration`: the heat a burnt portion of lava
    /// gives, 1 or more.
    pub heat_generation: u64,
    /// `heatGenerationLava`: heat a tick from each face touching lava,
    /// 0..=Long.MAX / 7.
    pub heat_generation_lava: u64,
    /// `heatGenerationNether`: heat a tick in an ultra-warm dimension, 1 or more.
    pub heat_generation_nether: u64,
    /// `tankCapacity`: the lava tank, in mB, 1..=Integer.MAX.
    pub heat_tank_capacity: u32,
    /// `heatGenerationFluidRate`: mB of lava a burn takes, 1..=the tank.
    pub heat_fluid_rate: u32,
    /// `generator-storage.toml` `solarGenerator`: Joules it holds, 1 or more.
    pub solar_storage: u64,
    /// `generator-storage.toml` `heatGenerator`: Joules it holds, 1 or more.
    pub heat_storage: u64,
}

impl Default for GeneratorsConfig {
    fn default() -> Self {
        Self {
            solar_generation: 50,
            heat_generation: 200,
            heat_generation_lava: 30,
            heat_generation_nether: 100,
            heat_tank_capacity: 24_000,
            heat_fluid_rate: 10,
            solar_storage: 96_000,
            heat_storage: 160_000,
        }
    }
}

impl GeneratorsConfig {
    /// From `generators.toml` and `generator-storage.toml` (whose keys sit at
    /// the top level).
    pub fn from_tables(generators: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let section = |name: &str| generators.and_then(|t| t.get(name)?.as_table());
        if let Some(solar) = section("solar_generator")
            && let Some(v) = positive(solar, "solarGeneration")
        {
            config.solar_generation = v;
        }
        if let Some(heat) = section("heat_generator") {
            if let Some(v) = positive(heat, "heatGeneration") {
                config.heat_generation = v;
            }
            if let Some(v) = heat.get("heatGenerationLava").and_then(toml::Value::as_integer)
                && (0..=i64::MAX / 7).contains(&v)
            {
                config.heat_generation_lava = v as u64;
            }
            if let Some(v) = positive(heat, "heatGenerationNether") {
                config.heat_generation_nether = v;
            }
            if let Some(v) = heat.get("tankCapacity").and_then(toml::Value::as_integer)
                && (1..=i64::from(i32::MAX)).contains(&v)
            {
                config.heat_tank_capacity = v as u32;
            }
            if let Some(v) = heat.get("heatGenerationFluidRate").and_then(toml::Value::as_integer)
                && (1..=i64::from(config.heat_tank_capacity)).contains(&v)
            {
                config.heat_fluid_rate = v as u32;
            }
        }
        if let Some(storage) = storage {
            if let Some(v) = positive(storage, "solarGenerator") {
                config.solar_storage = v;
            }
            if let Some(v) = positive(storage, "heatGenerator") {
                config.heat_storage = v;
            }
        }
        config
    }
}

/// The files `EnergyConfig::load` reads, under `<config>/Mekanism/`.
pub const FILES: [&str; 5] = ["general.toml", "tiers.toml", "common.toml", "generators.toml", "generator-storage.toml"];

/// When each of `FILES` last changed (none: missing).
pub type Stamp = [Option<std::time::SystemTime>; 5];

/// When each of `FILES` last changed: Mekanism's configs reload live
/// (NeoForge's file watcher), so a holder of the settings reads them again
/// when this changes.
pub fn stamp(config: &Path) -> Stamp {
    FILES.map(|name| std::fs::metadata(config.join("Mekanism").join(name)).and_then(|m| m.modified()).ok())
}

impl EnergyConfig {
    /// Reads `<config>/Mekanism/`'s `FILES`.
    pub fn load(config: &Path) -> Self {
        let read = |name: &str| {
            std::fs::read_to_string(config.join("Mekanism").join(name))
                .ok()
                .and_then(|text| text.parse::<toml::Table>().ok())
        };
        let mut config = Self::from_tables(read("general.toml").as_ref(), read("tiers.toml").as_ref());
        config.show_fe = shows_fe(read("common.toml").as_ref());
        config.generators =
            GeneratorsConfig::from_tables(read("generators.toml").as_ref(), read("generator-storage.toml").as_ref());
        config
    }

    pub fn from_tables(general: Option<&toml::Table>, tiers: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let section = |table: Option<&toml::Table>, path: &[&str]| -> Option<toml::Table> {
            let mut at = table?;
            for key in path {
                at = at.get(*key)?.as_table()?;
            }
            Some(at.clone())
        };
        if let Some(delay) = general.and_then(|g| g.get("blockDeactivationDelay")).and_then(toml::Value::as_integer)
            && (0..=1200).contains(&delay)
        {
            config.deactivation_delay = delay as u32;
        }
        if let Some(conversion) = section(general, &["energy_conversion"]) {
            if let Some(b) = conversion.get("blacklistForge").and_then(toml::Value::as_bool) {
                config.blacklist_forge = b;
            }
            if let Some(rate) = conversion.get("feConversionRate").and_then(float)
                && (1e-4..=1e4).contains(&rate)
            {
                config.fe_conversion_rate = rate;
            }
        }
        if let Some(cubes) = section(tiers, &["energy_cubes"]) {
            for (i, tier) in CubeTier::ALL.into_iter().enumerate() {
                if let Some(v) = positive(&cubes, &format!("{}Capacity", tier.name())) {
                    config.cubes[i].0 = v;
                }
                if let Some(v) = positive(&cubes, &format!("{}Output", tier.name())) {
                    config.cubes[i].1 = v;
                }
            }
        }
        if let Some(cables) = section(tiers, &["transmitters", "energy"]) {
            for (i, tier) in CableTier::ALL.into_iter().enumerate() {
                if let Some(v) = positive(&cables, &format!("{}Capacity", tier.name())) {
                    config.cables[i] = v;
                }
            }
        }
        config
    }

    pub fn cube(&self, tier: CubeTier) -> (u64, u64) {
        self.cubes[CubeTier::ALL.iter().position(|t| *t == tier).unwrap_or(0)]
    }

    pub fn cable(&self, tier: CableTier) -> u64 {
        self.cables[CableTier::ALL.iter().position(|t| *t == tier).unwrap_or(0)]
    }
}

/// `common.toml`'s `energyType` (top level): Forge Energy unless it says `JOULES`.
pub fn shows_fe(common: Option<&toml::Table>) -> bool {
    common.and_then(|c| c.get("energyType")).and_then(toml::Value::as_str) != Some("JOULES")
}

fn float(v: &toml::Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

/// A `long` setting of range 1..=Long.MAX_VALUE.
fn positive(table: &toml::Table, key: &str) -> Option<u64> {
    let v = table.get(key)?.as_integer()?;
    (v >= 1).then_some(v as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_files_are_read_again() {
        let dir = std::env::temp_dir().join(format!("mekanism-energy-config-{}", std::process::id()));
        let mek = dir.join("Mekanism");
        std::fs::create_dir_all(&mek).unwrap();
        assert_eq!(stamp(&dir), [None; 5]);
        assert_eq!(EnergyConfig::load(&dir), EnergyConfig::default());
        std::fs::write(mek.join("tiers.toml"), "[energy_cubes]\nbasicOutput = 5000\n").unwrap();
        let written = stamp(&dir);
        assert!(written[1].is_some() && written[0].is_none());
        assert_eq!(EnergyConfig::load(&dir).cube(CubeTier::Basic), (4_000_000, 5_000));
        std::fs::write(mek.join("generators.toml"), "[solar_generator]\nsolarGeneration = 150\n").unwrap();
        assert!(stamp(&dir)[3].is_some());
        assert_eq!(EnergyConfig::load(&dir).generators.solar_generation, 150);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atm10_files_override_the_defaults_and_bad_values_are_ignored() {
        let general: toml::Table = "[energy_conversion]\nblacklistForge = false\nfeConversionRate = 4.0\n"
            .parse()
            .unwrap();
        let tiers: toml::Table = "[energy_cubes]\nbasicCapacity = 5000000\nbasicOutput = 0\n\
             creativeCapacity = 9223372036854775807\n[transmitters]\n[transmitters.energy]\n\
             ultimateCapacity = 9000000\n"
            .parse()
            .unwrap();
        let c = EnergyConfig::from_tables(Some(&general), Some(&tiers));
        assert_eq!(c.fe_conversion_rate, 4.0);
        assert_eq!(c.cube(CubeTier::Basic), (5_000_000, 4_000));
        assert_eq!(c.cube(CubeTier::Creative).0, i64::MAX as u64);
        assert_eq!(c.cable(CableTier::Ultimate), 9_000_000);
        assert_eq!(c.cable(CableTier::Basic), 8_000);
        let wild: toml::Table = "[energy_conversion]\nfeConversionRate = 0.0\n".parse().unwrap();
        assert_eq!(EnergyConfig::from_tables(Some(&wild), None).fe_conversion_rate, 2.5);
        assert_eq!(EnergyConfig::from_tables(None, None), EnergyConfig::default());
        let delay: toml::Table = "blockDeactivationDelay = 20\n".parse().unwrap();
        assert_eq!(EnergyConfig::from_tables(Some(&delay), None).deactivation_delay, 20);
        let wild: toml::Table = "blockDeactivationDelay = 5000\n".parse().unwrap();
        assert_eq!(EnergyConfig::from_tables(Some(&wild), None).deactivation_delay, 60);
        let joules: toml::Table = "energyType = \"JOULES\"\n".parse().unwrap();
        let fe: toml::Table = "energyType = \"FORGE_ENERGY\"\n".parse().unwrap();
        assert!(!shows_fe(Some(&joules)));
        assert!(shows_fe(Some(&fe)) && shows_fe(None));
    }

    #[test]
    fn atm10_generator_files_override_the_defaults() {
        // ATM10 8.4's generators.toml and generator-storage.toml, the keys this port reads.
        let generators: toml::Table = "[solar_generator]\n\tsolarGeneration = 150\n\tadvancedSolarGeneration = 1200\n\
             [heat_generator]\n\theatGeneration = 800\n\theatGenerationLava = 30\n\theatGenerationNether = 100\n\
             \ttankCapacity = 24000\n\theatGenerationFluidRate = 10\n"
            .parse()
            .unwrap();
        let storage: toml::Table = "heatGenerator = 160000\nsolarGenerator = 96000\n".parse().unwrap();
        let g = GeneratorsConfig::from_tables(Some(&generators), Some(&storage));
        assert_eq!((g.solar_generation, g.heat_generation, g.heat_generation_lava, g.heat_generation_nether), (150, 800, 30, 100));
        assert_eq!((g.heat_tank_capacity, g.heat_fluid_rate, g.solar_storage, g.heat_storage), (24_000, 10, 96_000, 160_000));
        assert_eq!(GeneratorsConfig::from_tables(None, None), GeneratorsConfig::default());
        assert_eq!(GeneratorsConfig::default().solar_generation, 50, "Mekanism's own default");
        // Out of range: the rate may not pass the tank; lava heat may be 0, generation may not.
        let odd: toml::Table = "[heat_generator]\ntankCapacity = 100\nheatGenerationFluidRate = 101\n\
             heatGenerationLava = 0\nheatGeneration = 0\n"
            .parse()
            .unwrap();
        let g = GeneratorsConfig::from_tables(Some(&odd), None);
        assert_eq!((g.heat_tank_capacity, g.heat_fluid_rate, g.heat_generation_lava, g.heat_generation), (100, 10, 0, 200));
    }
}
