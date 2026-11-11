//! Mekanism's Resistive Heater (`TileEntityResistiveHeater`): it turns up to its energy usage of Joules a tick into
//! heat (`resistiveEfficiency` of each, `general.toml` `[heater]`) in a heat capacitor of 100 (`BasicHeatCapacitor`,
//! an inverse conduction of 5 and an insulation of 10), and hands heat to the heat handlers beside it
//! (`ITileHeatHandler.simulateAdjacent`: its temperature over the air's, over its conduction and theirs, times its
//! capacity) and loses some to the air on every side (`simulateEnvironment`). Its store holds 400 ticks of its usage
//! (`ResistiveHeaterEnergyContainer`, 100 J/t when placed), takes Joules from its left and right, and gives none.
//!
//! Heat a capacitor is handed waits until the end of the tick (`updateHeatCapacitors`), so everything in one tick
//! reads the temperature the tick started with.
use crate::heat::Ambient;
use crate::sides::{Direction, RelativeSide};
use std::path::Path;

/// `HEAT_CAPACITY`.
pub const HEAT_CAPACITY: f64 = 100.0;
/// `INVERSE_CONDUCTION_COEFFICIENT`.
pub const INVERSE_CONDUCTION: f64 = 5.0;
/// `INVERSE_INSULATION_COEFFICIENT`.
pub const INVERSE_INSULATION: f64 = 10.0;
/// `AttributeEnergy`'s usage for the heater: its usage when placed.
pub const BASE_USAGE: u64 = 100;
/// `ResistiveHeaterEnergyContainer.USAGE_MULTIPLIER`: its store holds this many ticks of its usage.
pub const USAGE_MULTIPLIER: u64 = 400;
/// `HeatAPI.AIR_INVERSE_COEFFICIENT`.
const AIR_INVERSE_COEFFICIENT: f64 = 10_000.0;
/// The heat a capacitor ignores (`1.0E-6F`).
const EPSILON: f64 = 1.0E-6_f32 as f64;
/// The sides its store takes Joules from (`EnergyContainerHelper.forSide`).
pub const ENERGY_SIDES: [RelativeSide; 2] = [RelativeSide::Left, RelativeSide::Right];

/// `general.toml` `[heater]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaterConfig {
    /// `resistiveEfficiency`: heat a Joule.
    pub efficiency: f64,
}

impl Default for HeaterConfig {
    fn default() -> Self {
        Self { efficiency: 0.6 }
    }
}

impl HeaterConfig {
    pub fn from_general(general: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        if let Some(v) = general
            .and_then(|g| g.get("heater"))
            .and_then(toml::Value::as_table)
            .and_then(|s| s.get("resistiveEfficiency"))
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .filter(|v| (0.0..=1.0).contains(v))
        {
            c.efficiency = v;
        }
        c
    }

    pub fn load(config: &Path) -> Self {
        let general = std::fs::read_to_string(config.join("Mekanism").join("general.toml")).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_general(general.as_ref())
    }
}

/// Whether its store takes Joules through `side` when it faces `facing`.
pub fn takes_energy(facing: Direction, side: Direction) -> bool {
    ENERGY_SIDES.contains(&RelativeSide::from_directions(facing, side))
}

/// `updateEnergyUsage`'s capacity: 400 ticks of its usage (`multiplyClamped`).
pub fn capacity(usage: u64) -> u64 {
    usage.saturating_mul(USAGE_MULTIPLIER).min(i64::MAX as u64)
}

/// A resistive heater as its block entity keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heater {
    /// Joules held.
    pub energy: u64,
    /// `energy_usage`: the Joules it turns into heat a tick.
    pub usage: u64,
    /// The capacitor's heat; none until it is first read (Mekanism's -1), when it starts at its air's.
    pub heat: Option<f64>,
    /// `heatToHandle`: heat handed to it (by another heater) that hasn't landed yet. Not saved.
    pub pending: f64,
}

impl Default for Heater {
    fn default() -> Self {
        Self { energy: 0, usage: BASE_USAGE, heat: None, pending: 0.0 }
    }
}

/// What a tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeaterTick {
    /// `clientEnergyUsed`: the Joules used (`setActive` when some).
    pub used: u64,
    /// The heat handed to the handler on each side (`Direction` order).
    pub handed: [f64; 6],
    /// `lastTransferLoss`: degrees sent to the handlers beside it.
    pub transfer_loss: f64,
    /// `lastEnvironmentLoss`: degrees lost to the air.
    pub environment_loss: f64,
}

impl Heater {
    pub fn capacity(&self) -> u64 {
        capacity(self.usage)
    }

    /// `getTotalTemperature`.
    pub fn temperature(&self, ambient: &Ambient) -> f64 {
        self.heat.unwrap_or(HEAT_CAPACITY * ambient.own) / HEAT_CAPACITY
    }

    /// `onUpdateServer` after the energy slot, then `updateHeatCapacitors`: up to its usage out of its store into
    /// heat, then `simulate`: each side's heat handler (`sinks`, its total inverse conduction) takes
    /// `(T - air) / (theirs + 5) * 100`, the air on each side `(T - air) / 10015 * 100`; what it was handed and
    /// gave lands at the end.
    pub fn tick(&mut self, config: &HeaterConfig, ambient: &Ambient, sinks: [Option<f64>; 6]) -> HeaterTick {
        let mut out = HeaterTick::default();
        let used = self.energy.min(self.usage);
        if used > 0 {
            self.pending += used as f64 * config.efficiency;
            self.energy -= used;
        }
        out.used = used;
        let stored = *self.heat.get_or_insert(HEAT_CAPACITY * ambient.own);
        let temperature = stored / HEAT_CAPACITY;
        for (i, sink) in sinks.iter().enumerate() {
            let Some(theirs) = sink else { continue };
            let transfer = (temperature - ambient.sides[i]) / (theirs + INVERSE_CONDUCTION);
            let heat = transfer * HEAT_CAPACITY;
            self.pending -= heat;
            out.handed[i] = heat;
            out.transfer_loss += transfer;
        }
        for side_air in ambient.sides {
            let transfer = (temperature - side_air) / (AIR_INVERSE_COEFFICIENT + INVERSE_INSULATION + INVERSE_CONDUCTION);
            self.pending -= transfer * HEAT_CAPACITY;
            if transfer > 0.0 {
                out.environment_loss += transfer;
            }
        }
        if self.pending != 0.0 && self.pending.abs() > EPSILON {
            self.heat = Some(stored + self.pending);
            self.pending = 0.0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAINS: f64 = 300.0 + 25.0 * (0.8_f32 as f64 - 0.8);

    #[test]
    fn alone_it_warms_as_on_the_oracle() {
        // The oracle's heater H0: 100 J/t, nothing beside it.
        let mut h = Heater { energy: 40_000, ..Heater::default() };
        let air = Ambient::uniform(PLAINS);
        let config = HeaterConfig::default();
        h.tick(&config, &air, [None; 6]);
        assert_eq!(h.heat, Some(30_060.000_029_802_322));
        h.tick(&config, &air, [None; 6]);
        assert_eq!(h.heat, Some(30_119.964_083_721_443));
        for _ in 2..10 {
            h.tick(&config, &air, [None; 6]);
        }
        assert!((h.heat.unwrap() - 30_598.385_037_696_793).abs() < 1e-6);
        assert_eq!(h.energy, 39_000);
    }

    #[test]
    fn beside_a_boiler_it_settles_where_the_oracle_s_did() {
        // 10 000 J/t, a formed boiler (inverse conduction 1) on one side: 6000 heat a tick in, about 5978.5 out.
        let mut h = Heater { energy: 4_000_000, usage: 10_000, heat: None, pending: 0.0 };
        let air = Ambient::uniform(PLAINS);
        let config = HeaterConfig::default();
        let sinks = [None, None, None, None, None, Some(1.0)];
        let first = h.tick(&config, &air, sinks);
        assert_eq!((first.used, first.handed[5]), (10_000, 0.0));
        assert_eq!(h.heat, Some(36_000.000_029_802_32));
        let second = h.tick(&config, &air, sinks);
        assert!((second.handed[5] - 1000.0).abs() < 1e-6);
        assert_eq!(h.heat, Some(40_996.405_421_714_46));
        for _ in 2..200 {
            h.tick(&config, &air, sinks);
        }
        assert!((h.heat.unwrap() - 65_871.057_636_014_62).abs() < 1e-6, "{:?}", h.heat);
        assert_eq!(h.energy, 2_000_000);
    }

    #[test]
    fn beside_an_evaporation_valve_it_warms_the_plant_as_on_the_oracle() {
        // A 4x4x3 plant in plains (1800 of heat capacity), the heater against a valve: what it hands lands at the
        // plant's next tick with the air's pull (`mekboil-work/b3.py`).
        use crate::evaporation::{self, EvaporationConfig, Heat};
        let atm10 = EvaporationConfig { temp_multiplier: 1.85, heat_capacity: 600.0, ..EvaporationConfig::default() };
        let mut plant = Heat::created(None, 3, PLAINS, &atm10);
        let mut h = Heater { energy: 4_000_000, usage: 10_000, heat: None, pending: 0.0 };
        let air = Ambient::uniform(PLAINS);
        let mut handed = 0.0;
        let mut rows = Vec::new();
        for _ in 0..20 {
            evaporation::heat_tick(&mut plant, PLAINS, 0, handed, &atm10);
            handed = h.tick(&HeaterConfig::default(), &air, [None, None, None, None, None, Some(1.0)]).handed[5];
            rows.push(plant.stored);
        }
        assert_eq!(rows[1], 540_000.000_536_441_8);
        assert_eq!(rows[2], 541_000.000_536_441_8);
        assert!((rows[3] - 542_805.901_952_697_2).abs() < 1e-6, "{}", rows[3]);
        assert!((rows[19] - 616_604.833_185_564).abs() < 1e-6, "{}", rows[19]);
    }

    #[test]
    fn its_store_and_sides() {
        assert_eq!(capacity(BASE_USAGE), 40_000);
        assert_eq!(capacity(u64::MAX), i64::MAX as u64);
        // Facing north: its left is east, its right west.
        assert!(takes_energy(Direction::North, Direction::East) && takes_energy(Direction::North, Direction::West));
        assert!(!takes_energy(Direction::North, Direction::South) && !takes_energy(Direction::North, Direction::Up));
        let t: toml::Table = "[heater]\nresistiveEfficiency = 0.6\n".parse().unwrap();
        assert_eq!(HeaterConfig::from_general(Some(&t)).efficiency, 0.6);
    }
}
