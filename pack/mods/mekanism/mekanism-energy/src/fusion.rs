//! Mekanism Generators' Fusion Reactor (`FusionReactorValidator`, `FusionReactorMultiblockData`, `FusionReactorCache`,
//! `TileEntityFusionReactorLogicAdapter`): a 5x5x5 whose faces leave their corners and most of their edges out
//! (`ALLOWED_GRID`: `fetchCuboid` over every side, missing at most 72 positions), frames in the faces' rings and the
//! edges' middles, the controller at the top's centre, ports, logic adapters, laser focus matrices and reactor glass in
//! the faces' middles, and air inside. Its plasma heats by laser Joules into a focus matrix (a tenth of them a kelvin
//! while it isn't burning); at 10^8 K a full Hohlraum in its slot ignites it, and while burning it injects up to its
//! injection rate of D-T fuel a tick from equal deuterium and tritium and burns what its plasma's heat above 10^8 K
//! allows, `fuelEnergy` a mB heating the plasma. The plasma heats the case, the case boils water into steam and gives
//! Joules to the air through its thermocouples (`generators.toml` `[fusion_reactor]`, `[hohlraum]`).
use crate::multiblock::{self, Cuboid, Failure, Formation, Inner, Node, Pos, Requirement, Side};
use std::collections::BTreeMap;
use std::path::Path;

/// `FusionReactorValidator.BOUNDS`: exactly 5 each way.
pub const SIZE: [i32; 3] = [5, 5, 5];
/// `precheck`'s tolerance.
pub const TOLERANCE: i32 = 72;
/// `ALLOWED_GRID`, by the face's horizontal then vertical coordinate: 0 ignored, 1 a frame (`FRAME`), 2 any block of
/// the structure (`OTHER`).
pub const ALLOWED_GRID: [[u8; 5]; 5] = [[0, 0, 1, 0, 0], [0, 1, 2, 1, 0], [1, 2, 2, 2, 1], [0, 1, 2, 1, 0], [0, 0, 1, 0, 0]];
/// `MAX_INJECTION`: the screen's two characters, even.
pub const MAX_INJECTION: i32 = 98;
/// `burnTemperature`.
pub const BURN_TEMPERATURE: f64 = 1.0E8;
/// `plasmaHeatCapacity`.
pub const PLASMA_HEAT_CAPACITY: f64 = 100.0;
/// `caseHeatCapacity`.
pub const CASE_HEAT_CAPACITY: f64 = 1.0;
/// `inverseInsulation`.
pub const INVERSE_INSULATION: f64 = 100_000.0;
/// `plasmaCaseConductivity`.
pub const PLASMA_CASE_CONDUCTIVITY: f64 = 0.2;
/// `HeatAPI.AIR_INVERSE_COEFFICIENT`.
const AIR_INVERSE_COEFFICIENT: f64 = 10_000.0;
/// `HeatAPI.EPSILON` (a float).
const EPSILON: f64 = 1.0E-6_f32 as f64;
/// The injection rate of a reactor that never had one (`FusionReactorCache.getInjectionRate` of -1).
pub const DEFAULT_INJECTION: i32 = 2;

/// `generators.toml` `[fusion_reactor]` and `[hohlraum]`, with `general.toml`'s water enthalpy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FusionConfig {
    pub energy_per_fuel: i64,
    pub thermocouple_efficiency: f64,
    pub casing_conductivity: f64,
    pub water_heating_ratio: f64,
    pub fuel_capacity: i64,
    pub energy_capacity: u64,
    pub water_per_injection: i32,
    pub steam_per_injection: i64,
    pub hohlraum_capacity: i64,
    pub hohlraum_fill_rate: i64,
    pub water_thermal_enthalpy: f64,
}

impl Default for FusionConfig {
    /// Mekanism's defaults (ATM10: 0.04, 0.333333333, 0.27272727272727 and 500 mB).
    fn default() -> Self {
        Self {
            energy_per_fuel: 10_000_000,
            thermocouple_efficiency: 0.05,
            casing_conductivity: 0.1,
            water_heating_ratio: 0.3,
            fuel_capacity: 1000,
            energy_capacity: 1_000_000_000,
            water_per_injection: 1_000_000,
            steam_per_injection: 100_000_000,
            hohlraum_capacity: 10,
            hohlraum_fill_rate: 1,
            water_thermal_enthalpy: 10.0,
        }
    }
}

impl FusionConfig {
    pub fn from_tables(generators: Option<&toml::Table>, general: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let float = |t: &toml::Table, key: &str, lo: f64, hi: f64| {
            t.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).filter(|v| (lo..=hi).contains(v))
        };
        let long = |t: &toml::Table, key: &str, lo: i64, hi: i64| t.get(key).and_then(toml::Value::as_integer).filter(|v| (lo..=hi).contains(v));
        if let Some(t) = generators.and_then(|g| g.get("fusion_reactor")).and_then(toml::Value::as_table) {
            if let Some(v) = long(t, "fuelEnergy", 1, i64::MAX) {
                c.energy_per_fuel = v;
            }
            if let Some(v) = float(t, "thermocoupleEfficiency", 0.01, 1.0) {
                c.thermocouple_efficiency = v;
            }
            if let Some(v) = float(t, "casingThermalConductivity", 0.001, 1.0) {
                c.casing_conductivity = v;
            }
            if let Some(v) = float(t, "waterHeatingRatio", 0.0, 1.0) {
                c.water_heating_ratio = v;
            }
            if let Some(v) = long(t, "fuelCapacity", 2, i64::MAX) {
                c.fuel_capacity = v;
            }
            if let Some(v) = long(t, "energyCapacity", 1, i64::MAX) {
                c.energy_capacity = v as u64;
            }
            if let Some(v) = long(t, "waterPerInjection", 1, i64::from(i32::MAX) / i64::from(MAX_INJECTION)) {
                c.water_per_injection = v as i32;
            }
            if let Some(v) = long(t, "steamPerInjection", 1, i64::MAX / i64::from(MAX_INJECTION)) {
                c.steam_per_injection = v;
            }
        }
        if let Some(t) = generators.and_then(|g| g.get("hohlraum")).and_then(toml::Value::as_table) {
            if let Some(v) = long(t, "capacity", 1, i64::MAX) {
                c.hohlraum_capacity = v;
            }
            if let Some(v) = long(t, "fillRate", 1, i64::MAX) {
                c.hohlraum_fill_rate = v;
            }
        }
        if let Some(v) = general
            .and_then(|g| g.get("energy_conversion"))
            .and_then(toml::Value::as_table)
            .and_then(|s| s.get("maxEnergyPerSteam"))
            .and_then(toml::Value::as_integer)
            .filter(|v| *v >= 0)
        {
            c.water_thermal_enthalpy = v as f64;
        }
        c
    }

    /// `<config>/Mekanism/generators.toml` and `general.toml`.
    pub fn load(config: &Path) -> Self {
        let read = |name: &str| std::fs::read_to_string(config.join("Mekanism").join(name)).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tables(read("generators.toml").as_ref(), read("general.toml").as_ref())
    }
}

// ---- Formation ----------------------------------------------------------------------------------------------

/// `FusionReactorValidator.getStructureRequirement`: a wall position by `ALLOWED_GRID` over the face it lies on, the
/// inside the cuboid's own.
pub fn requirement(cuboid: &Cuboid, p: Pos) -> Requirement {
    let d = [p[0] - cuboid.min[0], p[1] - cuboid.min[1], p[2] - cuboid.min[2]];
    let size = cuboid.size();
    let Some(axis) = (0..3).find(|&a| d[a] == 0 || d[a] == size[a] - 1) else { return cuboid.requirement(p) };
    let (h, v) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let at = |c: i32| usize::try_from(c).ok().filter(|c| *c < 5);
    match (at(d[h]), at(d[v])) {
        (Some(h), Some(v)) => match ALLOWED_GRID[h][v] {
            0 => Requirement::Ignored,
            1 => Requirement::Frame,
            _ => Requirement::Other,
        },
        _ => Requirement::Frame,
    }
}

/// `validateFrame`'s controller position: the top's centre.
pub fn controller_pos(cuboid: &Cuboid) -> Pos {
    [cuboid.min[0] + 2, cuboid.max[1], cuboid.min[2] + 2]
}

/// `FormationProtocol.doUpdate` with the `FusionReactorValidator`: the cuboid over every side within `TOLERANCE`, then
/// every block by `requirement`: the controller (`is_controller`) at the top's centre and nowhere else, air inside.
pub fn form(nodes: &BTreeMap<Pos, Node>, is_controller: impl Fn(Pos) -> bool, inner: impl FnMut(Pos) -> Inner) -> Result<Formation, Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = multiblock::fetch_cuboid_sides(nodes, SIZE, SIZE, &Side::ALL, TOLERANCE).ok_or(Failure::Fail)?;
    let at = controller_pos(&cuboid);
    let check = |p: Pos, _: Option<Node>| {
        if requirement(&cuboid, p) == Requirement::Ignored {
            return None;
        }
        match (p == at, is_controller(p)) {
            (true, false) => Some(Failure::NoController),
            (false, true) => Some(Failure::ControllerConflict(p)),
            _ => None,
        }
    };
    multiblock::validate_with(cuboid, nodes, |p| requirement(&cuboid, p), check, inner)
}

// ---- The reactor --------------------------------------------------------------------------------------------

/// The reactor's own state: the cache's (`plasma_temp`, `injection_rate`, `burning`) and what a formed reactor keeps
/// beside it (its last temperatures, the fuel it last burnt, its water and steam capacities, the heat it lost).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Core {
    pub plasma: f64,
    /// `injectionRate`: -1 in a cache that never had one.
    pub rate: i32,
    pub burning: bool,
    pub last_plasma: f64,
    pub last_case: f64,
    pub last_burned: i64,
    /// `maxWater` and `maxSteam`: set only when the injection rate changes (`setInjectionRate`), so a reactor at the
    /// default rate of 2 holds no water.
    pub max_water: i32,
    pub max_steam: i64,
    pub environment_loss: f64,
    pub transfer_loss: f64,
}

impl Core {
    /// A new multiblock at the biome's air (`FusionReactorMultiblockData`'s constructor).
    pub fn new(ambient: f64) -> Self {
        Self {
            plasma: ambient,
            rate: DEFAULT_INJECTION,
            burning: false,
            last_plasma: ambient,
            last_case: ambient,
            last_burned: 0,
            max_water: 0,
            max_steam: 0,
            environment_loss: 0.0,
            transfer_loss: 0.0,
        }
    }

    /// `FusionReactorCache.apply` onto a new multiblock: the plasma, the injection rate (-1 reads as 2) and the burning
    /// flag.
    pub fn applied(ambient: f64, plasma: f64, rate: i32, burning: bool, config: &FusionConfig) -> Self {
        let mut core = Self::new(ambient);
        core.plasma = plasma;
        core.set_rate(if rate == -1 { DEFAULT_INJECTION } else { rate }, config);
        core.burning = burning;
        core
    }

    /// `setInjectionRate`: a new rate sets the water and steam capacities (the tanks then cut to them by the caller).
    pub fn set_rate(&mut self, rate: i32, config: &FusionConfig) {
        if rate == self.rate {
            return;
        }
        self.rate = rate;
        self.max_water = rate.wrapping_mul(config.water_per_injection);
        self.max_steam = i64::from(rate).wrapping_mul(config.steam_per_injection);
    }

    /// `setInjectionRateFromPacket`: even, within 0 and 98.
    pub fn rate_from_packet(rate: i32) -> i32 {
        (rate - rate % 2).clamp(0, MAX_INJECTION)
    }

    /// `addTemperatureFromEnergyInput`: laser Joules into a focus matrix.
    pub fn add_energy(&mut self, joules: u64) {
        let e = joules.min(i64::MAX as u64) as f64;
        if self.burning {
            self.plasma += e / PLASMA_HEAT_CAPACITY;
        } else {
            self.plasma += e / PLASMA_HEAT_CAPACITY * 10.0;
        }
    }
}

/// The reactor's tanks and its case's heat (capacity 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tanks {
    pub deuterium: i64,
    pub tritium: i64,
    pub fuel: i64,
    pub steam: i64,
    pub water: i32,
    pub energy: u64,
    pub heat: f64,
}

fn clamp_to_long(d: f64) -> i64 {
    crate::clamp_to_long(d)
}

fn clamp_to_int(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// `MathUtils.multiplyClamped`.
fn multiply_clamped(a: i64, b: i64) -> i64 {
    a.checked_mul(b).unwrap_or(if (a < 0) == (b < 0) { i64::MAX } else { i64::MIN })
}

/// `injectFuel`: up to the rate of D-T fuel, from equal deuterium and tritium, as much as the fuel tank has room for.
fn inject(t: &mut Tanks, rate: i32, config: &FusionConfig) {
    let needed = (config.fuel_capacity - t.fuel).max(0);
    let available = 2 * t.deuterium.min(t.tritium);
    let mut amount = needed.min(available.min(i64::from(rate)));
    amount -= amount % 2;
    let half = amount / 2;
    t.deuterium -= half;
    t.tritium -= half;
    t.fuel += amount.max(0);
}

/// `burnFuel`: the plasma's last heat above 10^8 K in mB, at most the fuel there is; each mB heats the plasma.
fn burn(t: &mut Tanks, core: &mut Core, config: &FusionConfig) -> i64 {
    let wanted = (core.last_plasma - BURN_TEMPERATURE) * 1.0;
    let fuel = t.fuel as f64;
    let burned = clamp_to_long(if wanted < 0.0 { 0.0 } else { wanted.min(fuel) });
    t.fuel -= burned;
    core.plasma += multiply_clamped(burned, config.energy_per_fuel) as f64 / PLASMA_HEAT_CAPACITY;
    burned
}

/// The ports' air (`ITileHeatHandler.simulateEnvironment` of each port, every side of it): what the case loses to the
/// air at `ambient`, handed to `pending`, and the loss (positive parts) summed.
fn environment(case: f64, ports: usize, ambient: f64, pending: &mut f64, config: &FusionConfig) -> f64 {
    let inverse = AIR_INVERSE_COEFFICIENT + INVERSE_INSULATION + 1.0 / config.casing_conductivity;
    let mut loss = 0.0;
    for _ in 0..ports * 6 {
        let transfer = (case - ambient) / inverse;
        *pending += -transfer * CASE_HEAT_CAPACITY;
        if transfer > 0.0 {
            loss += transfer;
        }
    }
    loss
}

/// What a tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    /// A full Hohlraum ignited it and is used up.
    pub hohlraum_used: bool,
    pub burned: i64,
}

/// `FusionReactorMultiblockData.tick` less the ports' output: the ignition by a full Hohlraum (`hohlraum`: the D-T
/// fuel it holds, None unless full) at 10^8 K, the injection and the burn while burning, then `transferHeat` (plasma to
/// case, case to water, the ports' air, the thermocouples), the case's heat landing and the last temperatures.
pub fn tick(t: &mut Tanks, core: &mut Core, hohlraum: Option<i64>, ports: usize, ambient: f64, config: &FusionConfig) -> Tick {
    let mut out = Tick::default();
    if core.plasma >= BURN_TEMPERATURE {
        if !core.burning
            && let Some(fuel) = hohlraum
        {
            t.fuel += fuel.min((config.fuel_capacity - t.fuel).max(0)).max(0);
            core.last_plasma = core.plasma;
            core.burning = true;
            out.hohlraum_used = true;
        }
        if core.burning {
            inject(t, core.rate, config);
            out.burned = burn(t, core, config);
            if out.burned == 0 {
                core.burning = false;
            }
        }
    } else {
        core.burning = false;
    }
    core.last_burned = out.burned;
    // `transferHeat`.
    let mut pending = 0.0;
    let plasma_case = PLASMA_CASE_CONDUCTIVITY * (core.last_plasma - core.last_case);
    if plasma_case.abs() > EPSILON {
        core.plasma -= plasma_case / PLASMA_HEAT_CAPACITY;
        pending += plasma_case;
    }
    let case_water = config.water_heating_ratio * (core.last_case - ambient);
    if case_water.abs() > EPSILON {
        let boil = (crate::boiler::STEAM_ENERGY_EFFICIENCY * case_water / config.water_thermal_enthalpy) as i32;
        let boil = boil.min(t.water.min(clamp_to_int((core.max_steam - t.steam).max(0))));
        if boil > 0 {
            t.water -= boil;
            t.steam += i64::from(boil);
            pending += -(f64::from(boil) * config.water_thermal_enthalpy / crate::boiler::STEAM_ENERGY_EFFICIENCY);
        }
    }
    core.environment_loss = environment(t.heat / CASE_HEAT_CAPACITY, ports, ambient, &mut pending, config);
    core.transfer_loss = 0.0;
    let case_air = config.casing_conductivity * (core.last_case - ambient);
    if case_air.abs() > EPSILON {
        pending += -case_air;
        let joules = clamp_to_long(case_air * config.thermocouple_efficiency);
        if joules > 0 {
            t.energy = t.energy.saturating_add(joules as u64).min(config.energy_capacity.max(t.energy));
        }
    }
    // `updateHeatCapacitors`.
    if pending != 0.0 && pending.abs() > EPSILON {
        t.heat += pending;
    }
    core.last_plasma = core.plasma;
    core.last_case = t.heat / CASE_HEAT_CAPACITY;
    out
}

// ---- Its statistics (`GuiFusionReactorHeat`, `GuiFusionReactorStats`) and logic adapters ----------------------

fn k(active: bool, config: &FusionConfig) -> f64 {
    if active { config.water_heating_ratio } else { 0.0 }
}

/// `getIgnitionTemperature`.
pub fn ignition_temperature(active: bool, config: &FusionConfig) -> f64 {
    let (k, c, e) = (k(active, config), config.casing_conductivity, config.energy_per_fuel as f64);
    BURN_TEMPERATURE * e * 1.0 * (0.2 + k + c) / (e * 1.0 * (0.2 + k + c) - 0.2 * (k + c))
}

/// `getMaxPlasmaTemperature`.
pub fn max_plasma_temperature(core: &Core, active: bool, config: &FusionConfig) -> f64 {
    let (k, c) = (k(active, config), config.casing_conductivity);
    let burn = i64::from(core.rate).max(core.last_burned);
    (burn.wrapping_mul(config.energy_per_fuel)) as f64 / 0.2 * (0.2 + k + c) / (k + c)
}

/// `getMaxCasingTemperature`.
pub fn max_casing_temperature(core: &Core, active: bool, config: &FusionConfig) -> f64 {
    let burn = i64::from(core.rate).max(core.last_burned);
    multiply_clamped(config.energy_per_fuel, burn) as f64 / (k(active, config) + config.casing_conductivity)
}

/// `getMinInjectionRate`.
pub fn min_injection_rate(active: bool, config: &FusionConfig) -> i32 {
    let (k, c, e) = (k(active, config), config.casing_conductivity, config.energy_per_fuel as f64);
    let x = 2.0E7 * (k + c) / (e * 1.0 * (0.2 + k + c) - 0.2 * (k + c));
    2 * (x / 2.0).ceil() as i32
}

/// `getPassiveGeneration(active, current)`: the thermocouples' Joules a tick.
pub fn passive_generation(core: &Core, active: bool, current: bool, config: &FusionConfig) -> i64 {
    let temp = if current { core.last_case } else { max_casing_temperature(core, active, config) };
    clamp_to_long(temp * (config.thermocouple_efficiency * config.casing_conductivity))
}

/// `getSteamPerTick(current)`.
pub fn steam_per_tick(core: &Core, current: bool, config: &FusionConfig) -> i64 {
    let temp = if current { core.last_case } else { max_casing_temperature(core, true, config) };
    clamp_to_long(temp * (crate::boiler::STEAM_ENERGY_EFFICIENCY * config.water_heating_ratio) / config.water_thermal_enthalpy)
}

/// A logic adapter's mode (`FusionReactorLogic`, saved as `logic_type`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Logic {
    #[default]
    Disabled,
    Ready,
    Capacity,
    Depleted,
}

impl Logic {
    pub const ALL: [Self; 4] = [Self::Disabled, Self::Ready, Self::Capacity, Self::Depleted];

    pub fn by_id(id: i32) -> Self {
        Self::ALL.get(usize::try_from(id).unwrap_or(0)).copied().unwrap_or_default()
    }

    pub fn id(self) -> i32 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0) as i32
    }

    pub fn name(self) -> &'static str {
        ["disabled", "ready", "capacity", "depleted"][self.id() as usize]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.name() == name)
    }
}

/// `checkMode`: whether an adapter in `logic` (with `active_cooled`) gives its signal.
pub fn check(logic: Logic, cooled: bool, core: &Core, t: &Tanks, config: &FusionConfig) -> bool {
    match logic {
        Logic::Disabled => false,
        Logic::Ready => core.last_plasma >= ignition_temperature(cooled, config),
        Logic::Capacity => core.last_plasma >= max_plasma_temperature(core, cooled, config),
        Logic::Depleted => {
            let half = i64::from(core.rate / 2);
            t.fuel == 0 && (half == 0 || t.deuterium < half || t.tritium < half)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ATM10's `[fusion_reactor]`.
    const ATM10: FusionConfig = FusionConfig {
        energy_per_fuel: 10_000_000,
        thermocouple_efficiency: 0.04,
        casing_conductivity: 0.333333333,
        water_heating_ratio: 0.27272727272727,
        fuel_capacity: 500,
        energy_capacity: 1_000_000_000,
        water_per_injection: 1_000_000,
        steam_per_injection: 100_000_000,
        hohlraum_capacity: 10,
        hohlraum_fill_rate: 1,
        water_thermal_enthalpy: 10.0,
    };
    /// Plains (0.8f) as Mekanism reads it.
    const PLAINS: f64 = 300.0 + 25.0 * (0.8_f32 as f64 - 0.8);

    fn frame(o: Pos, other: &[(Pos, Node)]) -> BTreeMap<Pos, Node> {
        let c = Cuboid::new(o, [o[0] + 4, o[1] + 4, o[2] + 4]);
        let mut nodes = BTreeMap::new();
        for p in c.positions() {
            match requirement(&c, p) {
                Requirement::Frame => {
                    nodes.insert(p, Node::Frame);
                }
                Requirement::Other => {
                    nodes.insert(p, Node::Structural);
                }
                _ => {}
            }
        }
        nodes.insert(controller_pos(&c), Node::Other);
        for (p, n) in other {
            nodes.insert([o[0] + p[0], o[1] + p[1], o[2] + p[2]], *n);
        }
        nodes
    }

    #[test]
    fn a_5x5x5_with_its_corners_out_forms_with_its_controller_on_top() {
        let o = [0, 60, 0];
        let ctrl = [2, 64, 2];
        let nodes = frame(o, &[([0, 2, 2], Node::Valve)]);
        assert_eq!(nodes.len(), 12 + 24 + 30, "an edge's middle, a face's ring and a face's middle");
        let f = form(&nodes, |p| p == ctrl, |_| Inner::Air).unwrap();
        assert_eq!(f.cuboid.size(), [5, 5, 5]);
        assert_eq!(f.valves, vec![([0, 62, 2], crate::sides::Direction::West)]);
        assert_eq!(form(&nodes, |_| false, |_| Inner::Air), Err(Failure::NoController));
        assert_eq!(form(&nodes, |p| p == ctrl || p == [2, 62, 0], |_| Inner::Air), Err(Failure::ControllerConflict([2, 62, 0])));
        assert_eq!(form(&nodes, |p| p == ctrl, |p| if p == [2, 62, 2] { Inner::Invalid } else { Inner::Air }), Err(Failure::InvalidInner([2, 62, 2])));
        let mut glass_ring = nodes.clone();
        glass_ring.insert([1, 61, 0], Node::Structural);
        assert_eq!(form(&glass_ring, |p| p == ctrl, |_| Inner::Air), Err(Failure::InvalidFrame([1, 61, 0])), "a ring takes frames only");
        let mut corner = nodes.clone();
        corner.insert([0, 60, 0], Node::Frame);
        assert!(form(&corner, |p| p == ctrl, |_| Inner::Air).is_ok(), "a corner is ignored");
    }

    /// The oracle's E1 (rate 2, no water, three ports): 1.5 * 10^9 J of laser after the tick at 300 K, ignited by a
    /// Hohlraum of 10 mB the tick after, D and T kept full.
    #[test]
    fn ignition_and_burn_match_the_oracle() {
        let mut core = Core::new(PLAINS);
        let mut t = Tanks { deuterium: 500, tritium: 500, heat: PLAINS, ..Tanks::default() };
        tick(&mut t, &mut core, Some(10), 3, PLAINS, &ATM10);
        core.add_energy(1_500_000_000);
        assert_eq!(core.plasma, 150000300.0000003);
        let mut rows = Vec::new();
        for i in 0..4 {
            let out = tick(&mut t, &mut core, if i == 0 { Some(10) } else { None }, 3, PLAINS, &ATM10);
            assert_eq!(out.hohlraum_used, i == 0);
            t.deuterium = 500;
            t.tritium = 500;
            rows.push((core.plasma, t.heat, core.burning, t.energy));
        }
        assert_eq!(rows[0], (150900300.0000003, 30000300.000000298, true, 0));
        assert_eq!(rows[1], (150858500.0000003, 44175391.05297187, true, 399999));
        assert_eq!(rows[2], (150845133.78210625, 50779754.05198853, true, 989000));
        assert_eq!(rows[3], (150845003.022646, 53858036.192346826, true, 1666059));
    }

    #[test]
    fn a_new_rate_sets_the_water_and_steam_room() {
        let mut core = Core::applied(PLAINS, PLAINS, -1, false, &ATM10);
        assert_eq!((core.rate, core.max_water, core.max_steam), (2, 0, 0), "the default rate leaves them at 0");
        core.set_rate(Core::rate_from_packet(11), &ATM10);
        assert_eq!((core.rate, core.max_water, core.max_steam), (10, 10_000_000, 1_000_000_000));
        let mut cold = Core::new(PLAINS);
        cold.add_energy(1000);
        assert_eq!(cold.plasma, PLAINS + 100.0);
    }

    #[test]
    fn the_logic_adapter_modes() {
        let core = Core::new(PLAINS);
        let t = Tanks::default();
        assert!(check(Logic::Depleted, false, &core, &t, &ATM10));
        assert!(!check(Logic::Ready, false, &core, &t, &ATM10));
        assert!(!check(Logic::Depleted, false, &core, &Tanks { deuterium: 1, tritium: 1, ..t }, &ATM10));
        assert_eq!(Logic::from_name("capacity").map(Logic::id), Some(2));
    }
}
