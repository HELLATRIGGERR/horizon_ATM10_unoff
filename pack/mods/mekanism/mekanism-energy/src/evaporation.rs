//! Mekanism's Thermal Evaporation Plant (`EvaporationValidator`, `EvaporationMultiblockData`): a 4x4 cuboid of
//! thermal evaporation blocks, 3 to 18 high, open at the top (`fetchCuboid` over every side but the top, missing at
//! most 8 positions: the top corners may go), with valves, structural glass and one controller in its walls; the
//! top layer's corners are ignored, its edges take any casing, its middle is inside (air). Its input tank holds
//! `volume / 4 * fluidPerTank` mB of a fluid some `mekanism:evaporating` recipe takes, its output tank
//! `outputTankCapacity` mB. Its heat (`VariableHeatCapacitor`, `heatCapacity * height`) follows the air of its
//! corners' biomes (`heatDissipation * sqrt(difference)` a tick) and rises by `solarMultiplier` for each Advanced
//! Solar Generator on a top corner that sees the sun; above 300 K it runs its recipe
//! `(min(T, 3000) - 300) * tempMultiplier * height / 18` times a tick (`general.toml` `[thermal_evaporation]`).
use crate::multiblock::{self, Cuboid, Failure, Formation, Inner, Node, Pos, Requirement, Side, WallRelative};
use std::collections::BTreeMap;
use std::path::Path;

/// `EvaporationValidator.MIN_CUBOID` and `MAX_CUBOID` (length, height, width).
pub const MIN_SIZE: [i32; 3] = [4, 3, 4];
pub const MAX_SIZE: [i32; 3] = [4, 18, 4];
/// `EvaporationMultiblockData.MAX_HEIGHT`.
pub const MAX_HEIGHT: i32 = 18;
/// `MAX_MULTIPLIER_TEMP`.
pub const MAX_MULTIPLIER_TEMP: f64 = 3000.0;
/// `HeatAPI.AMBIENT_TEMP`: the temperature it starts producing above.
pub const AMBIENT: f64 = 300.0;
/// `precheck`'s tolerance: the planes may miss 8 positions (the top corners, each in two planes).
pub const TOLERANCE: i32 = 8;
/// The heat a capacitor ignores (`BasicHeatCapacitor.update`: `1.0E-6F`).
const EPSILON: f64 = 1.0E-6_f32 as f64;

/// `general.toml` `[thermal_evaporation]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaporationConfig {
    pub heat_dissipation: f64,
    pub solar_multiplier: f64,
    pub temp_multiplier: f64,
    pub heat_capacity: f64,
    pub fluid_per_tank: i32,
    pub output_tank_capacity: i32,
}

impl Default for EvaporationConfig {
    /// Mekanism's defaults (ATM10 sets `tempMultiplier` 1.85 and `heatCapacity` 600).
    fn default() -> Self {
        Self { heat_dissipation: 0.02, solar_multiplier: 0.2, temp_multiplier: 0.4, heat_capacity: 100.0, fluid_per_tank: 64_000, output_tank_capacity: 10_000 }
    }
}

impl EvaporationConfig {
    pub fn from_general(general: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let Some(s) = general.and_then(|g| g.get("thermal_evaporation")).and_then(toml::Value::as_table) else {
            return c;
        };
        let float = |key: &str, lo: f64, hi: f64| s.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).filter(|v| (lo..=hi).contains(v));
        if let Some(v) = float("heatDissipation", 0.001, 1000.0) {
            c.heat_dissipation = v;
        }
        if let Some(v) = float("solarMultiplier", 0.001, 1_000_000.0) {
            c.solar_multiplier = v;
        }
        if let Some(v) = float("tempMultiplier", 0.001, 1_000_000.0) {
            c.temp_multiplier = v;
        }
        if let Some(v) = float("heatCapacity", 1.0, 1_000_000.0) {
            c.heat_capacity = v;
        }
        // `fluidPerTank` 1..=Integer.MAX_VALUE / (MAX_HEIGHT * 4); `outputTankCapacity` at least 1.
        if let Some(v) = s.get("fluidPerTank").and_then(toml::Value::as_integer).filter(|v| (1..=i64::from(i32::MAX) / 72).contains(v)) {
            c.fluid_per_tank = v as i32;
        }
        if let Some(v) = s.get("outputTankCapacity").and_then(toml::Value::as_integer).filter(|v| (1..=i64::from(i32::MAX)).contains(v)) {
            c.output_tank_capacity = v as i32;
        }
        c
    }

    /// `<config>/Mekanism/general.toml`.
    pub fn load(config: &Path) -> Self {
        let general = std::fs::read_to_string(config.join("Mekanism").join("general.toml")).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_general(general.as_ref())
    }

    /// `setVolume`: the input tank's mB.
    pub fn input_capacity(&self, volume: i32) -> i32 {
        (volume.max(0) / 4).saturating_mul(self.fluid_per_tank)
    }
}

// ---- Formation ----------------------------------------------------------------------------------------------

/// `EvaporationValidator.getStructureRequirement`: on the top layer the corners are ignored, the edges take any
/// casing and the middle is inside; below, the cuboid's own.
pub fn requirement(cuboid: &Cuboid, p: Pos) -> Requirement {
    if p[1] == cuboid.max[1] {
        return match cuboid.wall_relative(p) {
            WallRelative::Corner => Requirement::Ignored,
            WallRelative::Edge => Requirement::Other,
            _ => Requirement::Inner,
        };
    }
    cuboid.requirement(p)
}

/// The sides `precheck` takes the cuboid from: all but the top.
pub const SIDES: [Side; 5] = [Side::BOTTOM, Side::NORTH, Side::SOUTH, Side::WEST, Side::EAST];

/// `FormationProtocol.doUpdate` with the `EvaporationValidator`: the open cuboid (no message when there is none),
/// every block of it with a second controller failing even on an ignored corner, and then a controller.
pub fn form(nodes: &BTreeMap<Pos, Node>, inner: impl FnMut(Pos) -> Inner) -> Result<Formation, Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = multiblock::fetch_cuboid_sides(nodes, MIN_SIZE, MAX_SIZE, &SIDES, TOLERANCE).ok_or(Failure::Fail)?;
    let mut found = false;
    let formation = multiblock::validate_with(
        cuboid,
        nodes,
        |p| requirement(&cuboid, p),
        |p, node| {
            let controller = node == Some(Node::Other);
            if found && controller {
                return Some(Failure::ControllerConflict(p));
            }
            found |= controller;
            None
        },
        inner,
    )?;
    if !found {
        return Err(Failure::NoController);
    }
    Ok(formation)
}

/// `updateSolars`: the top corners an Advanced Solar Generator heats it from (`IEvaporationSolar`): the max corner,
/// three west, three north, and both.
pub fn solar_spots(cuboid: &Cuboid) -> [Pos; 4] {
    let m = cuboid.max;
    [m, [m[0] - 3, m[1], m[2]], [m[0], m[1], m[2] - 3], [m[0] - 3, m[1], m[2] - 3]]
}

// ---- Heat ----------------------------------------------------------------------------------------------------

/// `calculateAverageAmbientTemperature`: `HeatAPI.getAmbientTemp` of the mean biome temperature of the cuboid's eight
/// corners (each a float, summed as doubles).
pub fn ambient(biome_temperatures: &[f32]) -> f64 {
    if biome_temperatures.is_empty() {
        return AMBIENT;
    }
    let mean = biome_temperatures.iter().map(|&t| f64::from(t)).sum::<f64>() / biome_temperatures.len() as f64;
    AMBIENT + 25.0 * (mean.clamp(-5.0, 5.0) - 0.8)
}

/// Its heat capacitor: the heat held (`storedHeat`) and the capacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heat {
    pub stored: f64,
    pub capacity: f64,
}

impl Heat {
    pub fn temperature(&self) -> f64 {
        self.stored / self.capacity
    }

    /// `onCreated` after `cache.apply`: the plant's capacity, `heatCapacity * height`, the difference to the cache's
    /// coming in at the plant's air (`setHeatCapacity(..., true)`). A new cache's capacitor holds no heat yet
    /// (`storedHeat` -1), so the plant's starts at its air the first time it is read: `capacity * ambient`.
    pub fn created(cached: Option<Heat>, height: i32, ambient: f64, config: &EvaporationConfig) -> Heat {
        let capacity = config.heat_capacity * f64::from(height);
        match cached {
            Some(old) => Heat { stored: old.stored + (capacity - old.capacity) * ambient, capacity },
            None => Heat { stored: capacity * ambient, capacity },
        }
    }

    /// `StorageUtils.mergeHeatCapacitors`: heat and capacity add up.
    pub fn merge(self, other: Heat) -> Heat {
        Heat { stored: self.stored + other.stored, capacity: self.capacity + other.capacity }
    }
}

/// `simulateEnvironment` then `updateHeatCapacitors`: `solars` Advanced Solar Generators that see the sun add
/// `solarMultiplier * capacity` each; within 0.001 K of the air it snaps to the air, else it moves
/// `heatDissipation * sqrt(difference)` degrees toward it; `handed` is the heat other blocks gave it since the last
/// tick. The degrees a tick it lost to the air (`lastEnvironmentLoss`).
pub fn heat_tick(heat: &mut Heat, ambient: f64, solars: u32, handed: f64, config: &EvaporationConfig) -> f64 {
    let temperature = heat.temperature();
    let capacity = heat.capacity;
    let mut to_handle = handed + f64::from(solars) * config.solar_multiplier * capacity;
    let mut loss = 0.0;
    if (temperature - ambient).abs() < 0.001 {
        to_handle += ambient * capacity - heat.stored;
    } else {
        let mut incr = config.heat_dissipation * (temperature - ambient).abs().sqrt();
        if temperature > ambient {
            incr = -incr;
        }
        to_handle += capacity * incr;
        if incr < 0.0 {
            loss = -incr;
        }
    }
    if to_handle != 0.0 && to_handle.abs() > EPSILON {
        heat.stored += to_handle;
    }
    loss
}

/// `tempMultiplier`: `(min(T, 3000) - 300) * tempMultiplier * height / 18`.
pub fn temp_multiplier(temperature: f64, height: i32, config: &EvaporationConfig) -> f64 {
    (temperature.min(MAX_MULTIPLIER_TEMP) - AMBIENT) * config.temp_multiplier * (f64::from(height) / f64::from(MAX_HEIGHT))
}

// ---- The recipe ---------------------------------------------------------------------------------------------

/// `setRequiredTicks`: below one operation a tick, one every `ceil(1 / multiplier)` ticks.
pub fn required_ticks(multiplier: f64) -> i32 {
    if multiplier > 0.0 && multiplier < 1.0 { (1.0 / multiplier).ceil() as i32 } else { 1 }
}

/// `setBaselineMaxOperations`: one below one a tick, else the multiplier's whole part (none or less when cold).
pub fn baseline_operations(multiplier: f64) -> i32 {
    if multiplier > 0.0 && multiplier < 1.0 { 1 } else { multiplier as i32 }
}

/// `setActive`'s `lastGain`: the operations a tick it runs at (a share below one), none while idle.
pub fn last_gain(multiplier: f64, active: bool) -> f64 {
    if !active {
        0.0
    } else if multiplier > 0.0 && multiplier < 1.0 {
        f64::from(1.0_f32 / (1.0 / multiplier).ceil() as f32)
    } else {
        multiplier
    }
}

/// The recipe errors its screen warns of (`TRACKED_ERROR_TYPES`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Errors {
    pub not_enough_input: bool,
    pub not_enough_output_space: bool,
    pub input_doesnt_produce_output: bool,
}

/// What a tick of `OneInputCachedRecipe.fluidToFluid` saw: the input held and what one operation takes, and the
/// output tank's room for the recipe's output and one operation's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Supply {
    pub input: i32,
    pub per_input: i32,
    pub room: i32,
    pub per_output: i32,
    /// The output tank holds nothing of the output's fluid and can't take it (`INPUT_DOESNT_PRODUCE_OUTPUT`).
    pub output_blocked: bool,
}

/// `CachedRecipe.process` with no energy: the operations a tick (the baseline, then what the input supports,
/// then the output's room); `operating` counts the ticks toward `required_ticks`. The operations that finish this
/// tick (their input used, their output made), whether it ran (`setActive`), and the errors.
pub fn process(operating: &mut i32, multiplier: f64, supply: Option<Supply>) -> (i32, bool, Errors) {
    let mut errors = Errors::default();
    let mut ops = baseline_operations(multiplier);
    match supply {
        // `mismatchedRecipe`: no input to match.
        None if ops > 0 => ops = -2,
        None => {}
        Some(s) if ops > 0 => {
            let can = if s.per_input > 0 { s.input / s.per_input } else { 0 };
            if can > 0 {
                ops = ops.min(can);
            } else {
                ops = -1;
                errors.not_enough_input = true;
            }
            if ops > 0 && s.per_output > 0 {
                let fit = s.room.max(0) / s.per_output;
                ops = ops.min(fit);
                if fit == 0 {
                    if s.output_blocked {
                        errors.input_doesnt_produce_output = true;
                    } else {
                        errors.not_enough_output_space = true;
                    }
                }
            }
        }
        Some(_) => {}
    }
    if ops > 0 {
        *operating += 1;
        if *operating >= required_ticks(multiplier) {
            *operating = 0;
            return (ops, true, errors);
        }
        return (0, true, errors);
    }
    if ops < 0 {
        *operating = 0;
    }
    (0, false, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATM10: EvaporationConfig =
        EvaporationConfig { heat_dissipation: 0.02, solar_multiplier: 0.2, temp_multiplier: 1.85, heat_capacity: 600.0, fluid_per_tank: 64_000, output_tank_capacity: 10_000 };

    /// A 4x4 plant at the origin, `h` high: base and walls of blocks, a controller at (1, 1, 0), the top's middle open.
    fn plant(h: i32) -> BTreeMap<Pos, Node> {
        let mut nodes = BTreeMap::new();
        for x in 0..4 {
            for y in 0..h {
                for z in 0..4 {
                    let wall = y == 0 || x == 0 || x == 3 || z == 0 || z == 3;
                    if wall {
                        nodes.insert([x, y, z], Node::Frame);
                    }
                }
            }
        }
        nodes.insert([1, 1, 0], Node::Other);
        nodes
    }

    fn air(_: Pos) -> Inner {
        Inner::Air
    }

    #[test]
    fn forms_at_three_and_eighteen_high_and_not_beyond() {
        for h in [3, 4, 18] {
            let f = form(&plant(h), air).unwrap_or_else(|e| panic!("{h}: {e:?}"));
            assert_eq!(f.cuboid, Cuboid::new([0, 0, 0], [3, h - 1, 3]));
        }
        assert_eq!(form(&plant(2), air), Err(Failure::Fail));
        assert_eq!(form(&plant(19), air), Err(Failure::Fail));
    }

    #[test]
    fn the_oracle_s_cases() {
        // The top corners may go; glass, valves and the controller may sit on the top ring; dirt inside, a block in
        // the top's middle or touching it from above, a second controller and a controller on an edge don't form.
        let mut no_corners = plant(4);
        for p in [[0, 3, 0], [3, 3, 0], [0, 3, 3], [3, 3, 3]] {
            no_corners.remove(&p);
        }
        assert!(form(&no_corners, air).is_ok());
        let mut ring = plant(4);
        ring.insert([1, 3, 0], Node::Structural);
        ring.insert([2, 3, 0], Node::Valve);
        ring.insert([2, 2, 0], Node::Structural);
        assert!(form(&ring, air).is_ok());
        let mut top_controller = plant(4);
        top_controller.insert([1, 1, 0], Node::Frame);
        top_controller.insert([1, 3, 0], Node::Other);
        assert!(form(&top_controller, air).is_ok());
        let mut two = plant(3);
        two.insert([2, 1, 0], Node::Other);
        assert_eq!(form(&two, air), Err(Failure::ControllerConflict([2, 1, 0])));
        let mut edge = plant(4);
        edge.insert([1, 1, 0], Node::Frame);
        edge.insert([0, 1, 0], Node::Other);
        assert_eq!(form(&edge, air), Err(Failure::InvalidFrame([0, 1, 0])));
        let mut none = plant(4);
        none.insert([1, 1, 0], Node::Frame);
        assert_eq!(form(&none, air), Err(Failure::NoController));
        let dirt = |p: Pos| if p == [1, 1, 1] { Inner::Invalid } else { Inner::Air };
        assert_eq!(form(&plant(4), dirt), Err(Failure::InvalidInner([1, 1, 1])));
        let mut middle = plant(4);
        middle.insert([1, 3, 1], Node::Frame);
        let block = |p: Pos| if p == [1, 3, 1] { Inner::Invalid } else { Inner::Air };
        assert_eq!(form(&middle, block), Err(Failure::InvalidInner([1, 3, 1])));
        let mut above = plant(4);
        above.insert([0, 4, 1], Node::Frame);
        assert_eq!(form(&above, air), Err(Failure::Fail));
        // Five wide.
        let mut wide = plant(4);
        for y in 0..4 {
            for z in 0..4 {
                wide.insert([4, y, z], Node::Frame);
            }
        }
        assert_eq!(form(&wide, air), Err(Failure::Fail));
    }

    #[test]
    fn heat_at_formation_as_the_oracle_saved_it() {
        // Plains (0.8f): 540000.0005364418 for 3 high; a desert (2.0): 330 K a few ticks later, snapped.
        let plains = ambient(&[0.8; 8]);
        let h = Heat::created(None, 3, plains, &ATM10);
        assert_eq!(h.capacity, 1800.0);
        assert_eq!(h.stored, 540_000.000_536_441_8);
        let desert = ambient(&[2.0; 8]);
        assert_eq!(desert, 330.0);
        let mut h = Heat::created(None, 3, desert, &ATM10);
        for _ in 0..20 {
            heat_tick(&mut h, desert, 0, 0.0, &ATM10);
        }
        assert_eq!(h.stored, 594_000.0);
        assert_eq!(temp_multiplier(h.temperature(), 3, &ATM10) as i32, 9);
        assert_eq!(temp_multiplier(330.0, 18, &ATM10) as i32, 55);
    }

    #[test]
    fn solars_heat_it_and_the_air_cools_it() {
        let mut h = Heat { stored: 300.0 * 1800.0, capacity: 1800.0 };
        let loss = heat_tick(&mut h, 300.0, 4, 0.0, &ATM10);
        assert_eq!(loss, 0.0);
        assert!((h.temperature() - 300.8).abs() < 1e-9);
        let loss = heat_tick(&mut h, 300.0, 0, 0.0, &ATM10);
        assert!((loss - 0.02 * 0.8_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn operations_follow_the_multiplier() {
        let supply = Some(Supply { input: 768_000, per_input: 10, room: 10_000, per_output: 1, output_blocked: false });
        let mut ticks = 0;
        assert_eq!(process(&mut ticks, 9.25, supply).0, 9);
        // A quarter: one every four ticks.
        let got: Vec<i32> = (0..8).map(|_| process(&mut ticks, 0.25, supply).0).collect();
        assert_eq!(got, [0, 0, 0, 1, 0, 0, 0, 1]);
        assert_eq!(last_gain(0.3, true), 0.25);
        // Cold: nothing, the progress reset.
        ticks = 2;
        assert_eq!(process(&mut ticks, -3.0, supply), (0, false, Errors::default()));
        assert_eq!(ticks, 0);
        // Not enough input, then a full output.
        let low = Some(Supply { input: 9, ..supply.unwrap() });
        assert!(process(&mut ticks, 9.25, low).2.not_enough_input);
        let full = Some(Supply { room: 0, ..supply.unwrap() });
        assert_eq!(process(&mut ticks, 9.25, full).0, 0);
        assert_eq!(EvaporationConfig::default().input_capacity(48), 768_000);
    }
}
