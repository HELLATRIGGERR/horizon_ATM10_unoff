//! `OritechConfig`: the numbers the processing machines run on, from `oritech-common.toml` (ATM10 8.4 ships one:
//! the pulverizer takes 64 RF a tick there, the foundry 256 and the fragment forge 512, twice the mod's defaults). A
//! missing file or entry keeps the mod's default.
use crate::machine::Kind;
use std::path::Path;

/// `BasicMachineConfig`: a machine's energy buffer, how much it takes a call and what a tick of work costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineSettings {
    pub energy_capacity: i64,
    pub max_energy_insertion: i64,
    pub max_energy_extraction: i64,
    pub energy_per_tick: i64,
}

impl MachineSettings {
    const fn new(capacity: i64, insertion: i64, extraction: i64, per_tick: i64) -> Self {
        Self { energy_capacity: capacity, max_energy_insertion: insertion, max_energy_extraction: extraction, energy_per_tick: per_tick }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub pulverizer: MachineSettings,
    pub fragment_forge: MachineSettings,
    pub assembler: MachineSettings,
    pub foundry: MachineSettings,
    pub centrifuge: MachineSettings,
    pub furnace: MachineSettings,
    /// `furnaceData.speedMultiplier`: the powered furnace's time against vanilla's (0.5: twice as fast).
    pub furnace_speed: f64,
    pub basic_generator: MachineSettings,
    pub bio_generator: MachineSettings,
    /// `additiveAddons`: addon bonuses add up (true) or multiply.
    pub additive_addons: bool,
    /// `layeredExtenders`.
    pub layered_extenders: bool,
    /// `addonConfig.burstAddonSpeedMultiplier` and `burstAddonThrottleMultiplier`.
    pub burst_speed_multiplier: f64,
    pub burst_throttle_multiplier: f64,
}

impl Default for Config {
    /// `ProcessingMachines`' defaults in `OritechConfig` (`new BasicMachineConfig(b, name, capacity, insertion, 0,
    /// perTick)`) and `AddonConfig`'s.
    fn default() -> Self {
        Self {
            pulverizer: MachineSettings::new(25_000, 32 * 8, 0, 32),
            fragment_forge: MachineSettings::new(50_000, 256 * 8, 0, 256),
            assembler: MachineSettings::new(50_000, 128 * 8, 0, 128),
            foundry: MachineSettings::new(50_000, 128 * 8, 0, 128),
            centrifuge: MachineSettings::new(10_000, 64 * 8, 0, 64),
            furnace: MachineSettings::new(10_000, 32 * 8, 0, 32),
            furnace_speed: 0.5,
            basic_generator: MachineSettings::new(50_000, 0, 32 * 8, 32),
            bio_generator: MachineSettings::new(100_000, 0, 64 * 8, 64),
            additive_addons: true,
            layered_extenders: false,
            burst_speed_multiplier: 8.0,
            burst_throttle_multiplier: 1.2,
        }
    }
}

impl Config {
    /// Reads `oritech-common.toml` of the config folder `dir`.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("oritech-common.toml")).map_or_else(|_| Self::default(), |t| Self::parse(&t))
    }

    /// The settings of one machine.
    pub fn machine(&self, kind: Kind) -> MachineSettings {
        match kind {
            Kind::Pulverizer => self.pulverizer,
            Kind::FragmentForge => self.fragment_forge,
            Kind::Assembler => self.assembler,
            Kind::Foundry => self.foundry,
            Kind::Centrifuge => self.centrifuge,
            Kind::PoweredFurnace => self.furnace,
            Kind::BasicGenerator => self.basic_generator,
            Kind::BioGenerator => self.bio_generator,
        }
    }

    fn machine_mut(&mut self, section: &str) -> Option<&mut MachineSettings> {
        match section {
            "pulverizerData" => Some(&mut self.pulverizer),
            "fragmentForgeData" => Some(&mut self.fragment_forge),
            "assemblerData" => Some(&mut self.assembler),
            "foundryData" => Some(&mut self.foundry),
            "centrifugeData" => Some(&mut self.centrifuge),
            "furnaceData" => Some(&mut self.furnace),
            "basicGeneratorData" => Some(&mut self.basic_generator),
            "bioGeneratorData" => Some(&mut self.bio_generator),
            _ => None,
        }
    }

    /// The TOML NeoForge writes: `[machineSettings.processingMachines.pulverizerData]` then `energyPerTick = 64`. Only
    /// the keys read here are looked at; a value that doesn't read keeps the default.
    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        let mut section = String::new();
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or_default().trim();
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = head.trim().to_owned();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            let last = section.rsplit('.').next().unwrap_or_default().to_owned();
            if last == "furnaceData" && key == "speedMultiplier" {
                c.furnace_speed = value.parse().unwrap_or(c.furnace_speed);
                continue;
            }
            if let Some(m) = c.machine_mut(&last) {
                let Ok(v) = value.parse::<i64>() else { continue };
                match key {
                    "energyCapacity" => m.energy_capacity = v.max(0),
                    "maxEnergyInsertion" => m.max_energy_insertion = v.max(0),
                    "maxEnergyExtraction" => m.max_energy_extraction = v.max(0),
                    "energyPerTick" => m.energy_per_tick = v.clamp(0, i64::from(i32::MAX)),
                    _ => {}
                }
                continue;
            }
            match key {
                "additiveAddons" => c.additive_addons = value.parse().unwrap_or(c.additive_addons),
                "layeredExtenders" => c.layered_extenders = value.parse().unwrap_or(c.layered_extenders),
                "burstAddonSpeedMultiplier" => c.burst_speed_multiplier = value.parse().unwrap_or(c.burst_speed_multiplier),
                "burstAddonThrottleMultiplier" => c.burst_throttle_multiplier = value.parse().unwrap_or(c.burst_throttle_multiplier),
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
    fn the_defaults_are_the_mods() {
        let c = Config::default();
        assert_eq!(c.pulverizer, MachineSettings::new(25_000, 256, 0, 32));
        assert_eq!(c.fragment_forge, MachineSettings::new(50_000, 2048, 0, 256));
        assert_eq!((c.assembler.energy_per_tick, c.foundry.energy_per_tick), (128, 128));
        assert_eq!(c.centrifuge, MachineSettings::new(10_000, 512, 0, 64));
        assert_eq!((c.furnace, c.furnace_speed), (MachineSettings::new(10_000, 256, 0, 32), 0.5));
        assert_eq!(c.basic_generator, MachineSettings::new(50_000, 0, 256, 32));
        assert_eq!(c.bio_generator, MachineSettings::new(100_000, 0, 512, 64));
        assert!(c.additive_addons && !c.layered_extenders);
    }

    #[test]
    fn a_pack_file_overrides_what_it_names() {
        let text = "additiveAddons = true\n[machineSettings]\n\t[machineSettings.processingMachines]\n\t\t[machineSettings.processingMachines.pulverizerData]\n\t\t\t#Energy capacity\n\t\t\tenergyCapacity = 25000\n\t\t\tmaxEnergyInsertion = 256\n\t\t\tenergyPerTick = 64\n\t\t[machineSettings.processingMachines.foundryData]\n\t\t\tenergyPerTick = 256\n\t\t[machineSettings.processingMachines.refineryData]\n\t\t\tenergyPerTick = 999\n";
        let c = Config::parse(text);
        assert_eq!(c.pulverizer, MachineSettings::new(25_000, 256, 0, 64));
        assert_eq!(c.foundry.energy_per_tick, 256);
        assert_eq!(c.assembler, Config::default().assembler, "a section it doesn't name stays");
    }

    /// ATM10's own file, when `ORITECH_CONFIG` names it (`config/oritech-common.toml` of the instance).
    #[test]
    fn atm10s_file_reads() {
        let Ok(path) = std::env::var("ORITECH_CONFIG") else { return };
        let c = Config::parse(&std::fs::read_to_string(path).expect("ORITECH_CONFIG reads"));
        assert_eq!(c.pulverizer, MachineSettings::new(25_000, 256, 0, 64));
        assert_eq!(c.fragment_forge, MachineSettings::new(50_000, 2048, 0, 512));
        assert_eq!(c.assembler, MachineSettings::new(50_000, 1024, 0, 128));
        assert_eq!(c.foundry, MachineSettings::new(50_000, 1024, 0, 256));
        assert_eq!(c.centrifuge, MachineSettings::new(10_000, 512, 0, 128));
        assert_eq!((c.furnace, c.furnace_speed), (MachineSettings::new(10_000, 256, 0, 64), 0.5));
        assert_eq!(c.basic_generator, MachineSettings::new(50_000, 0, 256, 64));
        assert_eq!(c.bio_generator, MachineSettings::new(400_000, 0, 2048, 256));
        assert!(c.additive_addons);
    }
}
