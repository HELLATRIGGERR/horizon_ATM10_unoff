//! The energy settings of Mekanism's config files, as ATM10 ships them in
//! `config/Mekanism/`: `general.toml` (`[energy_conversion]`,
//! `blockDeactivationDelay`) and `tiers.toml` (`[energy_cubes]`,
//! `[transmitters.energy]`), the unit screens show from `common.toml`
//! (`energyType`), and Mekanism Generators' `generators.toml`
//! (`[solar_generator]`, `[heat_generator]`) and `generator-storage.toml`, and
//! the machines' Joules a tick and store (`machine-usage.toml`,
//! `machine-storage.toml`, with `general.toml`'s `maxUpgradeMultiplier`). A
//! missing file, key or a value out of the mod's range keeps the default, as
//! NeoForge's config spec corrects it.
use crate::machine::MachineKind;
use crate::tier::{CableTier, ChemicalTankTier, CubeTier, TubeTier};
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
    /// `[chemical_tanks]` `<tier>Capacity` and `<tier>Output` of each chemical
    /// tank tier (mB, mB a tick), in `ChemicalTankTier::ALL` order.
    pub chemical_tanks: [(i64, i64); 5],
    /// `[transmitters.chemical]` `<tier>Capacity` and `<tier>PullAmount` of
    /// each pressurized tube tier (mB, mB a tick), in `TubeTier::ALL` order.
    pub tubes: [(i64, i64); 4],
    /// `energyType`: screens show Forge Energy (`FORGE_ENERGY`, the
    /// default) or Joules (`JOULES`).
    pub show_fe: bool,
    /// `blockDeactivationDelay`: ticks before a block that stopped working
    /// shows it (`active=false`), 0..=1200.
    pub deactivation_delay: u32,
    /// Mekanism Generators' settings.
    pub generators: GeneratorsConfig,
    /// The processing machines' settings.
    pub machines: MachinesConfig,
    /// The Electric Pump's and the Fluidic Plenisher's (`general.toml`
    /// `[pump]`, their Joules a tick and store).
    pub pump: crate::pump::PumpConfig,
    /// `general.toml` `[radiation]` and `common.toml` `enableDecayTimers`.
    pub radiation: crate::radiation::RadiationConfig,
}

impl Default for EnergyConfig {
    fn default() -> Self {
        Self {
            blacklist_forge: false,
            fe_conversion_rate: 2.5,
            cubes: CubeTier::ALL.map(|t| (t.default_capacity(), t.default_output())),
            cables: CableTier::ALL.map(CableTier::default_capacity),
            chemical_tanks: ChemicalTankTier::ALL.map(|t| (t.default_storage(), t.default_output())),
            tubes: TubeTier::ALL.map(|t| (t.default_capacity(), t.default_pull())),
            show_fe: true,
            deactivation_delay: 60,
            generators: GeneratorsConfig::default(),
            machines: MachinesConfig::default(),
            pump: crate::pump::PumpConfig::default(),
            radiation: crate::radiation::RadiationConfig::default(),
        }
    }
}

/// `machine-usage.toml` and `machine-storage.toml` (`UsageConfig`,
/// `StorageConfig`, whose keys sit at the top level) for the machines this
/// port has, and `general.toml`'s `maxUpgradeMultiplier`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MachinesConfig {
    /// Joules a tick of each machine, in `MachineKind::ALL` order, 0 or more.
    pub usage: [u64; MachineKind::ALL.len()],
    /// Joules each machine holds, 1 or more.
    pub storage: [u64; MachineKind::ALL.len()],
    /// `maxUpgradeMultiplier`: how far eight upgrades of a kind go, more than 1.
    pub max_upgrade_multiplier: f64,
    /// `general.toml` `[auto_eject] chemical`: mB a tick a block ejects of
    /// each chemical tank, and the most a tank dumping excess loses a tick, 1 or more.
    pub chemical_auto_eject: i64,
    /// `[auto_eject] dumpExcessKeepRatio`: what a tank dumping excess keeps, 0.001..=1.
    pub dump_excess_keep_ratio: f64,
    /// `[auto_eject] fluid`: mB a tick a block ejects of each fluid tank, 1 or more.
    pub fluid_auto_eject: i32,
    /// `machine-usage.toml` `randomizedConsumption`: the purification and
    /// injection chambers draw their chemical a tick at random (Mekanism's
    /// default, which ATM10's file, written before the option, leaves as it is).
    pub randomized_consumption: bool,
}

impl Default for MachinesConfig {
    fn default() -> Self {
        Self {
            usage: MachineKind::ALL.map(MachineKind::default_usage),
            storage: MachineKind::ALL.map(MachineKind::default_storage),
            max_upgrade_multiplier: 10.0,
            chemical_auto_eject: 1024,
            dump_excess_keep_ratio: 0.85,
            fluid_auto_eject: 1024,
            randomized_consumption: true,
        }
    }
}

impl MachinesConfig {
    pub fn from_tables(general: Option<&toml::Table>, usage: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        if let Some(v) = general.and_then(|g| g.get("maxUpgradeMultiplier")).and_then(toml::Value::as_integer)
            && v > 1
        {
            config.max_upgrade_multiplier = v as f64;
        }
        let eject = general.and_then(|g| g.get("auto_eject")).and_then(toml::Value::as_table);
        if let Some(v) = eject.and_then(|e| e.get("chemical")).and_then(toml::Value::as_integer)
            && v >= 1
        {
            config.chemical_auto_eject = v;
        }
        if let Some(v) = eject.and_then(|e| e.get("fluid")).and_then(toml::Value::as_integer)
            && (1..=i64::from(i32::MAX)).contains(&v)
        {
            config.fluid_auto_eject = v as i32;
        }
        if let Some(v) = eject.and_then(|e| e.get("dumpExcessKeepRatio")).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            && (0.001..=1.0).contains(&v)
        {
            config.dump_excess_keep_ratio = v;
        }
        if let Some(v) = usage.and_then(|t| t.get("randomizedConsumption")).and_then(toml::Value::as_bool) {
            config.randomized_consumption = v;
        }
        for (i, kind) in MachineKind::ALL.into_iter().enumerate() {
            if let Some(v) = usage.and_then(|t| t.get(kind.config_key())).and_then(toml::Value::as_integer)
                && v >= 0
            {
                config.usage[i] = v as u64;
            }
            if let Some(v) = storage.and_then(|t| positive(t, kind.config_key())) {
                config.storage[i] = v;
            }
        }
        config
    }

    fn index(kind: MachineKind) -> usize {
        MachineKind::ALL.iter().position(|k| *k == kind).unwrap_or(0)
    }

    /// The machine's base Joules a tick and store.
    pub fn of(&self, kind: MachineKind) -> (u64, u64) {
        let i = Self::index(kind);
        (self.usage[i], self.storage[i])
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
    /// `[gas_generator] tankCapacity`: the Gas-Burning Generator's fuel tank,
    /// in mB, 1 or more.
    pub gas_tank_capacity: i64,
    /// `[solar_generator] advancedSolarGeneration`: the Advanced Solar
    /// Generator's Joules a tick at peak, 0 or more.
    pub advanced_solar_generation: u64,
    /// `[bio_generator] bioGeneration`: Joules a tick a mB of bioethanol makes, 0 or more.
    pub bio_generation: u64,
    /// `[bio_generator] tankCapacity`: the bioethanol tank, in mB, 1..=Integer.MAX.
    pub bio_tank_capacity: u32,
    /// `[wind_generator] generationMin` and `generationMax`: Joules a tick
    /// at `minY` and at `maxY`, 0 or more.
    pub wind_generation_min: u64,
    pub wind_generation_max: u64,
    /// `[wind_generator] minY` (`DimensionType.MIN_Y..MAX_Y - 1`) and `maxY`
    /// (above `minY`): the heights between which the wind grows.
    pub wind_min_y: i32,
    pub wind_max_y: i32,
    /// `generator-storage.toml` `bioGenerator`, `advancedSolarGenerator` and
    /// `windGenerator`: Joules each holds, 1 or more.
    pub bio_storage: u64,
    pub advanced_solar_storage: u64,
    pub wind_storage: u64,
}

/// `DimensionType.MIN_Y` and `MAX_Y`: the range a wind height may take.
pub const DIMENSION_MIN_Y: i32 = -2032;
pub const DIMENSION_MAX_Y: i32 = 2031;

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
            gas_tank_capacity: crate::gas::TANK,
            advanced_solar_generation: 300,
            bio_generation: 350,
            bio_tank_capacity: 24_000,
            wind_generation_min: 60,
            wind_generation_max: 480,
            wind_min_y: 24,
            wind_max_y: DIMENSION_MAX_Y,
            bio_storage: 160_000,
            advanced_solar_storage: 200_000,
            wind_storage: 200_000,
        }
    }
}

impl GeneratorsConfig {
    /// From `generators.toml` and `generator-storage.toml` (whose keys sit at
    /// the top level).
    pub fn from_tables(generators: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let section = |name: &str| generators.and_then(|t| t.get(name)?.as_table());
        if let Some(solar) = section("solar_generator") {
            if let Some(v) = positive(solar, "solarGeneration") {
                config.solar_generation = v;
            }
            if let Some(v) = non_negative(solar, "advancedSolarGeneration") {
                config.advanced_solar_generation = v;
            }
        }
        if let Some(bio) = section("bio_generator") {
            if let Some(v) = non_negative(bio, "bioGeneration") {
                config.bio_generation = v;
            }
            if let Some(v) = bio.get("tankCapacity").and_then(toml::Value::as_integer)
                && (1..=i64::from(i32::MAX)).contains(&v)
            {
                config.bio_tank_capacity = v as u32;
            }
        }
        if let Some(wind) = section("wind_generator") {
            if let Some(v) = non_negative(wind, "generationMin") {
                config.wind_generation_min = v;
            }
            if let Some(v) = non_negative(wind, "generationMax") {
                config.wind_generation_max = v;
            }
            if let Some(v) = wind.get("minY").and_then(toml::Value::as_integer)
                && (i64::from(DIMENSION_MIN_Y)..i64::from(DIMENSION_MAX_Y)).contains(&v)
            {
                config.wind_min_y = v as i32;
            }
            // `value > windGenerationMinY.getOrDefault()`: an `Integer` above the minimum.
            if let Some(v) = wind.get("maxY").and_then(toml::Value::as_integer)
                && v > i64::from(config.wind_min_y)
                && v <= i64::from(i32::MAX)
            {
                config.wind_max_y = v as i32;
            }
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
        if let Some(gas) = section("gas_generator")
            && let Some(v) = positive(gas, "tankCapacity")
        {
            config.gas_tank_capacity = v.min(i64::MAX as u64) as i64;
        }
        if let Some(storage) = storage {
            if let Some(v) = positive(storage, "solarGenerator") {
                config.solar_storage = v;
            }
            if let Some(v) = positive(storage, "heatGenerator") {
                config.heat_storage = v;
            }
            if let Some(v) = positive(storage, "bioGenerator") {
                config.bio_storage = v;
            }
            if let Some(v) = positive(storage, "advancedSolarGenerator") {
                config.advanced_solar_storage = v;
            }
            if let Some(v) = positive(storage, "windGenerator") {
                config.wind_storage = v;
            }
        }
        config
    }
}

/// The files `EnergyConfig::load` reads, under `<config>/Mekanism/`.
pub const FILES: [&str; 7] = [
    "general.toml",
    "tiers.toml",
    "common.toml",
    "generators.toml",
    "generator-storage.toml",
    "machine-usage.toml",
    "machine-storage.toml",
];

/// When each of `FILES` last changed (none: missing).
pub type Stamp = [Option<std::time::SystemTime>; FILES.len()];

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
        config.machines = MachinesConfig::from_tables(
            read("general.toml").as_ref(),
            read("machine-usage.toml").as_ref(),
            read("machine-storage.toml").as_ref(),
        );
        config.pump = crate::pump::PumpConfig::from_tables(
            read("general.toml").as_ref(),
            read("machine-usage.toml").as_ref(),
            read("machine-storage.toml").as_ref(),
        );
        config.radiation = crate::radiation::RadiationConfig::from_tables(read("general.toml").as_ref(), read("common.toml").as_ref());
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
        if let Some(tanks) = section(tiers, &["chemical_tanks"]) {
            for (i, tier) in ChemicalTankTier::ALL.into_iter().enumerate() {
                if let Some(v) = positive(&tanks, &format!("{}Capacity", tier.name())) {
                    config.chemical_tanks[i].0 = v as i64;
                }
                if let Some(v) = positive(&tanks, &format!("{}Output", tier.name())) {
                    config.chemical_tanks[i].1 = v as i64;
                }
            }
        }
        if let Some(tubes) = section(tiers, &["transmitters", "chemical"]) {
            for (i, tier) in TubeTier::ALL.into_iter().enumerate() {
                if let Some(v) = positive(&tubes, &format!("{}Capacity", tier.name())) {
                    config.tubes[i].0 = v as i64;
                }
                if let Some(v) = positive(&tubes, &format!("{}PullAmount", tier.name())) {
                    config.tubes[i].1 = v as i64;
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

    /// A chemical tank tier's mB and output a tick.
    pub fn chemical_tank(&self, tier: ChemicalTankTier) -> (i64, i64) {
        self.chemical_tanks[tier as usize]
    }

    /// A pressurized tube tier's capacity and pull a tick.
    pub fn tube(&self, tier: TubeTier) -> (i64, i64) {
        self.tubes[tier as usize]
    }
}

/// `common.toml`'s `energyType` (top level): Forge Energy unless it says `JOULES`.
pub fn shows_fe(common: Option<&toml::Table>) -> bool {
    common.and_then(|c| c.get("energyType")).and_then(toml::Value::as_str) != Some("JOULES")
}

fn float(v: &toml::Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

/// A `long` setting of range 0..=Long.MAX_VALUE (`CachedLongValue.definePositive`).
fn non_negative(table: &toml::Table, key: &str) -> Option<u64> {
    let v = table.get(key)?.as_integer()?;
    (v >= 0).then_some(v as u64)
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
        assert_eq!(stamp(&dir), [None; FILES.len()]);
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
        assert_eq!(c.chemical_tank(ChemicalTankTier::Basic), (64_000, 1_000), "Mekanism's defaults where tiers.toml says nothing");
        let chem: toml::Table = "[chemical_tanks]
basicCapacity = 64000
basicOutput = 1000
creativeCapacity = 9223372036854775807
             creativeOutput = 4611686018427387903
eliteOutput = 0
[transmitters]
[transmitters.chemical]
basicCapacity = 4000
             basicPullAmount = 750
ultimateCapacity = 2000000
eliteCapacity = -1
"
            .parse()
            .unwrap();
        let c = EnergyConfig::from_tables(None, Some(&chem));
        assert_eq!(c.chemical_tank(ChemicalTankTier::Creative), (i64::MAX, 4_611_686_018_427_387_903));
        assert_eq!(c.chemical_tank(ChemicalTankTier::Elite), (1_024_000, 128_000), "0 is out of range");
        assert_eq!(c.tube(TubeTier::Basic), (4_000, 750));
        assert_eq!(c.tube(TubeTier::Ultimate), (2_000_000, 256_000));
        assert_eq!(c.tube(TubeTier::Elite), (256_000, 64_000));
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
    fn atm10_machine_files_override_the_defaults() {
        // ATM10 8.4's general.toml, machine-usage.toml and machine-storage.toml, the keys this port reads.
        let general: toml::Table = "maxUpgradeMultiplier = 16
[auto_eject]
fluid = 1024
chemical = 1024
dumpExcessKeepRatio = 0.85
".parse().unwrap();
        let usage: toml::Table = "enrichmentChamber = 50
crusher = 50
metallurgicInfuser = 50
energizedSmelter = 50
".parse().unwrap();
        let storage: toml::Table = "enrichmentChamber = 20000
crusher = 20000
metallurgicInfuser = 20000
energizedSmelter = 20000
".parse().unwrap();
        let m = MachinesConfig::from_tables(Some(&general), Some(&usage), Some(&storage));
        assert_eq!(m.max_upgrade_multiplier, 16.0);
        assert_eq!((m.chemical_auto_eject, m.dump_excess_keep_ratio, m.fluid_auto_eject), (1024, 0.85, 1024));
        assert_eq!(m.of(MachineKind::ElectrolyticSeparator), (400, 160_000), "Mekanism's own defaults");
        assert_eq!(m.of(MachineKind::Crusher), (50, 20_000));
        assert_eq!(MachinesConfig::from_tables(None, None, None).max_upgrade_multiplier, 10.0, "Mekanism's own default");
        let odd: toml::Table = "crusher = 0
".parse().unwrap();
        let free = MachinesConfig::from_tables(None, Some(&odd), Some(&odd));
        assert_eq!(free.of(MachineKind::Crusher), (0, 20_000), "usage may be 0, storage may not");
        let one: toml::Table = "maxUpgradeMultiplier = 1
".parse().unwrap();
        assert_eq!(MachinesConfig::from_tables(Some(&one), None, None).max_upgrade_multiplier, 10.0);
        // ATM10's machine-usage.toml has no `randomizedConsumption`: Mekanism's default, on.
        let atm10: toml::Table = "osmiumCompressor = 100\npurificationChamber = 200\nchemicalInjectionChamber = 400\n".parse().unwrap();
        let m = MachinesConfig::from_tables(None, Some(&atm10), None);
        assert!(m.randomized_consumption);
        assert_eq!(m.of(MachineKind::ChemicalInjectionChamber), (400, 160_000));
        let off: toml::Table = "randomizedConsumption = false\n".parse().unwrap();
        assert!(!MachinesConfig::from_tables(None, Some(&off), None).randomized_consumption);
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
        assert_eq!(g.gas_tank_capacity, 18_000, "Mekanism's default where ATM10's file says the same");
        let small: toml::Table = "[gas_generator]\ntankCapacity = 500\n".parse().unwrap();
        assert_eq!(GeneratorsConfig::from_tables(Some(&small), None).gas_tank_capacity, 500);
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

    #[test]
    fn atm10_bio_wind_and_advanced_solar_settings() {
        // ATM10 8.4's [bio_generator], [wind_generator] and their storage.
        let generators: toml::Table = "[solar_generator]\nadvancedSolarGeneration = 1200\n\
             [bio_generator]\nbioGeneration = 350\ntankCapacity = 24000\n\
             [wind_generator]\ngenerationMin = 150\ngenerationMax = 900\nminY = 24\nmaxY = 2031\n"
            .parse()
            .unwrap();
        let storage: toml::Table = "bioGenerator = 160000\nadvancedSolarGenerator = 2500000\nwindGenerator = 200000\n".parse().unwrap();
        let g = GeneratorsConfig::from_tables(Some(&generators), Some(&storage));
        assert_eq!((g.advanced_solar_generation, g.bio_generation, g.bio_tank_capacity), (1200, 350, 24_000));
        assert_eq!((g.wind_generation_min, g.wind_generation_max, g.wind_min_y, g.wind_max_y), (150, 900, 24, 2031));
        assert_eq!((g.bio_storage, g.advanced_solar_storage, g.wind_storage), (160_000, 2_500_000, 200_000));
        let d = GeneratorsConfig::default();
        assert_eq!((d.advanced_solar_generation, d.wind_generation_min, d.wind_generation_max, d.advanced_solar_storage), (300, 60, 480, 200_000));
        // A generation of 0 is allowed (`definePositive` is 0..); a maxY not above minY isn't, nor a minY at MAX_Y.
        let odd: toml::Table = "[bio_generator]\nbioGeneration = 0\ntankCapacity = 0\n[wind_generator]\nminY = 100\nmaxY = 100\n".parse().unwrap();
        let g = GeneratorsConfig::from_tables(Some(&odd), None);
        assert_eq!((g.bio_generation, g.bio_tank_capacity, g.wind_min_y, g.wind_max_y), (0, 24_000, 100, 2031));
        let top: toml::Table = "[wind_generator]\nminY = 2031\n".parse().unwrap();
        assert_eq!(GeneratorsConfig::from_tables(Some(&top), None).wind_min_y, 24);
    }
}
