//! Mekanism's Thermoelectric Boiler (`BoilerValidator`, `BoilerMultiblockData`): a cuboid of boiler casings, 3x3x3
//! to 18x18x18, with boiler valves and structural glass in its walls; inside, one full layer of pressure dispersers
//! splits the water below from the steam above, and superheating elements, all touching, sit below it. Its water
//! volume is what the air and its walls hold below the dispersers (`FormationProtocol.explore` from its last
//! position, its bottom wall too), its steam volume the layers from the dispersers' up to its top wall
//! (`general.toml` `[boiler]`: so many mB a block of each). Its heat (`VariableHeatCapacitor`, 50 a wall block)
//! comes from heat handlers beside it (the resistive heater) and from heated coolant (superheated sodium cooling into
//! its cooled tank, `HeatedCoolant`), and goes into boiling above 373.15 K at
//! `(T - 373.15) * capacity * waterConductivity` a tick, at most `superheatingHeatTransfer` an element, 50 heat a mB
//! of water (`maxEnergyPerSteam / 0.2`). Its valves take water and heated coolant in their input mode and give steam
//! or cooled coolant in theirs (`BoilerValveMode`).
use crate::evaporation::Heat;
use crate::multiblock::{self, Cuboid, Failure, Formation, Inner, Node, Pos};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::Path;

/// `HeatUtils.BASE_BOIL_TEMP`: 100 °C.
pub const BASE_BOIL_TEMP: f64 = 273.15 + 100.0;
/// `HeatUtils.getSteamEnergyEfficiency`.
pub const STEAM_ENERGY_EFFICIENCY: f64 = 0.2;
/// `CASING_HEAT_CAPACITY`: the heat capacity of each block of its walls.
pub const CASING_HEAT_CAPACITY: f64 = 50.0;
/// `CASING_INVERSE_CONDUCTION_COEFFICIENT`: what a heat handler beside it sees.
pub const INVERSE_CONDUCTION: f64 = 1.0;
/// `simulateEnvironment`'s `invConduction` (written out in Mekanism: the air's 10 000, its insulation 100 000 and its
/// conduction 1).
const ENVIRONMENT_INVERSE_CONDUCTION: f64 = 110_001.0;
/// The heat a capacitor ignores (`BasicHeatCapacitor.update`: `1.0E-6F`).
const EPSILON: f64 = 1.0E-6_f32 as f64;
/// `FormationProtocol.explore`'s `maxCount`.
const EXPLORE_LIMIT: usize = 5832;
/// `GeneralConfig`'s `maxVolume` (18³), which bounds the per-block amounts.
const MAX_VOLUME: i64 = 18 * 18 * 18;

/// `general.toml` `[boiler]`, and `[energy_conversion]` `maxEnergyPerSteam` (`HeatUtils.getWaterThermalEnthalpy`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoilerConfig {
    pub water_per_tank: i32,
    pub steam_per_tank: i64,
    pub heated_coolant_per_tank: i64,
    pub cooled_coolant_per_tank: i64,
    pub water_conductivity: f64,
    pub superheating_heat_transfer: f64,
    pub water_thermal_enthalpy: f64,
}

impl Default for BoilerConfig {
    /// Mekanism's defaults (ATM10 doubles the four per-block amounts).
    fn default() -> Self {
        Self {
            water_per_tank: 16_000,
            steam_per_tank: 160_000,
            heated_coolant_per_tank: 256_000,
            cooled_coolant_per_tank: 256_000,
            water_conductivity: 0.7,
            superheating_heat_transfer: 1.6e7,
            water_thermal_enthalpy: 10.0,
        }
    }
}

impl BoilerConfig {
    pub fn from_general(general: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let Some(g) = general else { return c };
        if let Some(s) = g.get("boiler").and_then(toml::Value::as_table) {
            let long = |key: &str, lo: i64, hi: i64| s.get(key).and_then(toml::Value::as_integer).filter(|v| (lo..=hi).contains(v));
            let float = |key: &str, lo: f64, hi: f64| s.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).filter(|v| (lo..=hi).contains(v));
            if let Some(v) = long("waterPerTank", 1, i64::from(i32::MAX) / MAX_VOLUME) {
                c.water_per_tank = v as i32;
            }
            if let Some(v) = long("steamPerTank", 10, i64::MAX / MAX_VOLUME) {
                c.steam_per_tank = v;
            }
            if let Some(v) = long("heatedCoolantPerTank", 1, i64::MAX / MAX_VOLUME) {
                c.heated_coolant_per_tank = v;
            }
            if let Some(v) = long("cooledCoolantPerTank", 1, i64::MAX / MAX_VOLUME) {
                c.cooled_coolant_per_tank = v;
            }
            if let Some(v) = float("waterConductivity", 0.01, 1.0) {
                c.water_conductivity = v;
            }
            if let Some(v) = float("superheatingHeatTransfer", 0.1, 1.024e9) {
                c.superheating_heat_transfer = v;
            }
        }
        if let Some(v) = g.get("energy_conversion").and_then(toml::Value::as_table).and_then(|s| s.get("maxEnergyPerSteam")).and_then(toml::Value::as_integer).filter(|v| *v >= 0) {
            c.water_thermal_enthalpy = v as f64;
        }
        c
    }

    /// `<config>/Mekanism/general.toml`.
    pub fn load(config: &Path) -> Self {
        let general = std::fs::read_to_string(config.join("Mekanism").join("general.toml")).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_general(general.as_ref())
    }

    /// `setWaterVolume` and `setSteamVolume`: the four tanks' capacities.
    pub fn capacities(&self, shape: &Shape) -> Capacities {
        let water = i64::from(shape.water_volume.max(0));
        let steam = i64::from(shape.steam_volume.max(0));
        Capacities {
            water: (water * i64::from(self.water_per_tank)).min(i64::from(i32::MAX)) as i32,
            steam: steam.saturating_mul(self.steam_per_tank),
            heated: water.saturating_mul(self.heated_coolant_per_tank),
            cooled: steam.saturating_mul(self.cooled_coolant_per_tank),
        }
    }

    /// `getBoilCapacity`: the most it can boil a tick with its elements.
    pub fn boil_capacity(&self, elements: i32) -> i64 {
        let boil = self.superheating_heat_transfer * f64::from(elements) / self.water_thermal_enthalpy;
        crate::clamp_to_long(boil * STEAM_ENERGY_EFFICIENCY)
    }
}

// ---- Formation ----------------------------------------------------------------------------------------------

/// What the validator finds inside a boiler at a position (`validateInner`): air, a pressure disperser or a
/// superheating element (`IInternalMultiblock`), anything else, or a position whose chunk isn't there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inside {
    Air,
    Disperser,
    Element,
    Invalid,
    Unloaded,
}

/// What `postcheck` works out of a formed boiler: its water and steam volumes (in blocks), its superheating
/// elements and the height of its disperser layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub water_volume: i32,
    pub steam_volume: i32,
    pub elements: i32,
    pub disperser_y: i32,
}

/// The four tanks' capacities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capacities {
    pub water: i32,
    pub steam: i64,
    pub heated: i64,
    pub cooled: i64,
}

/// `FormationProtocol.explore` counting: the positions `ok` takes that touch `start` (the start first), until 5832.
pub(crate) fn explore_count(start: Pos, mut ok: impl FnMut(Pos) -> bool) -> i32 {
    if !ok(start) {
        return 0;
    }
    let mut seen: HashSet<Pos> = HashSet::from([start]);
    let mut open = VecDeque::from([start]);
    while let Some(p) = open.pop_front() {
        if seen.len() >= EXPLORE_LIMIT {
            return seen.len() as i32;
        }
        for d in crate::sides::Direction::ALL {
            let o = d.offset();
            let next = [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
            if !seen.contains(&next) && ok(next) {
                seen.insert(next);
                open.push_back(next);
            }
        }
    }
    seen.len() as i32
}

/// `FormationProtocol.doUpdate` with the `BoilerValidator`: the cuboid (no message when there is none), every block
/// of it (dispersers and elements inside, besides air), then `postcheck`: a disperser layer filling the inside at the
/// height of the first disperser met (x, then y, then z), no other disperser, the elements all touching below it,
/// and no air below it that its walls don't reach.
pub fn form(nodes: &BTreeMap<Pos, Node>, mut inside: impl FnMut(Pos) -> Inside) -> Result<(Formation, Shape), Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = multiblock::fetch_cuboid(nodes, multiblock::MIN_BOUNDS, multiblock::MAX_BOUNDS).ok_or(Failure::Fail)?;
    let mut kinds: BTreeMap<Pos, Inside> = BTreeMap::new();
    let formation = multiblock::validate(cuboid, nodes, |p| {
        let kind = inside(p);
        kinds.insert(p, kind);
        match kind {
            Inside::Air => Inner::Air,
            Inside::Disperser | Inside::Element => Inner::Internal,
            Inside::Invalid => Inner::Invalid,
            Inside::Unloaded => Inner::Unloaded,
        }
    })?;
    let shape = postcheck(&cuboid, &formation.internal, &kinds)?;
    Ok((formation, shape))
}

/// `BoilerValidator.postcheck` over the inside `kinds` of a validated `cuboid` (its walls are all frame-compatible).
fn postcheck(cuboid: &Cuboid, internal: &[Pos], kinds: &BTreeMap<Pos, Inside>) -> Result<Shape, Failure> {
    let kind = |p: Pos| kinds.get(&p).copied();
    let dispersers: Vec<Pos> = internal.iter().copied().filter(|p| kind(*p) == Some(Inside::Disperser)).collect();
    let elements: Vec<Pos> = internal.iter().copied().filter(|p| kind(*p) == Some(Inside::Element)).collect();
    let (Some(&init), Some(&first_element)) = (dispersers.first(), elements.first()) else {
        return Err(if dispersers.is_empty() { Failure::NoDisperser } else { Failure::InvalidSuperheating });
    };
    let min = cuboid.min;
    let (length, height, width) = (cuboid.length(), cuboid.height(), cuboid.width());
    let mut left: BTreeSet<Pos> = dispersers.iter().copied().collect();
    for x in 1..length - 1 {
        for z in 1..width - 1 {
            let p = [min[0] + x, init[1], min[2] + z];
            if kind(p) != Some(Inside::Disperser) {
                return Err(Failure::MissingDisperser(p));
            }
            left.remove(&p);
        }
    }
    if !left.is_empty() {
        return Err(Failure::ExtraDisperser);
    }
    let superheating = explore_count(first_element, |p| p[1] < init[1] && kind(p) == Some(Inside::Element));
    if elements.len() as i32 > superheating {
        return Err(Failure::InvalidSuperheating);
    }
    // `isAirOrFrame`: inside, air; on the walls, the boiler's own blocks.
    let air_or_frame = |p: Pos| match cuboid.relative(p) {
        multiblock::Relative::Inside => kind(p) == Some(Inside::Air),
        multiblock::Relative::Walls => true,
        multiblock::Relative::Outside => false,
    };
    // `renderLocation` is a block above the cuboid's corner: the count starts above the bottom wall, the explore
    // takes it in (`y >= renderLocation.y - 1`).
    let mut total_air = 0;
    let mut init_air = None;
    for x in min[0]..min[0] + length {
        for y in min[1] + 1..init[1] {
            for z in min[2]..min[2] + width {
                if air_or_frame([x, y, z]) {
                    init_air = Some([x, y, z]);
                    total_air += 1;
                }
            }
        }
    }
    let in_water = |p: Pos| {
        p[1] >= min[1] && p[1] < init[1] && (min[0]..min[0] + length).contains(&p[0]) && (min[2]..min[2] + width).contains(&p[2]) && air_or_frame(p)
    };
    let water_volume = init_air.map_or(0, |start| explore_count(start, in_water));
    if total_air > water_volume {
        return Err(Failure::AirPockets);
    }
    let steam_height = min[1] + 1 + height - 2 - init[1];
    Ok(Shape { water_volume, steam_volume: width * length * steam_height, elements: superheating, disperser_y: init[1] })
}

// ---- The valves ----------------------------------------------------------------------------------------------

/// `BoilerValveMode`, the valve block's `mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValveMode {
    #[default]
    Input,
    OutputSteam,
    OutputCoolant,
}

impl ValveMode {
    pub const ALL: [Self; 3] = [Self::Input, Self::OutputSteam, Self::OutputCoolant];

    /// `getSerializedName`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::OutputSteam => "output_steam",
            Self::OutputCoolant => "output_coolant",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    /// `getNext`.
    pub fn next(self) -> Self {
        match self {
            Self::Input => Self::OutputSteam,
            Self::OutputSteam => Self::OutputCoolant,
            Self::OutputCoolant => Self::Input,
        }
    }

    /// Its lang entry (`boiler.mekanism.valve_mode_*`).
    pub fn text(self) -> &'static str {
        match self {
            Self::Input => "input only",
            Self::OutputSteam => "output steam",
            Self::OutputCoolant => "output coolant",
        }
    }
}

// ---- The tick --------------------------------------------------------------------------------------------------

/// What its tanks hold, in mB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tanks {
    pub water: i32,
    pub steam: i64,
    /// The heated coolant (`superheatedCoolantTank`).
    pub heated: i64,
    /// The cooled coolant (`cooledCoolantTank`).
    pub cooled: i64,
}

/// The heated coolant in its tank (`HeatedCoolant`: superheated sodium is 100 000 K, 5 heat a mB, a conductivity of
/// 0.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatedCoolant {
    pub temperature: f64,
    pub thermal_enthalpy: f64,
    pub conductivity: f64,
}

/// What a tick did, for its screen and its superheating elements.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    /// `hotMap`: at least 373.14 K as the tick began (its elements light up).
    pub hot: bool,
    /// `lastEnvironmentLoss`: degrees lost to the air.
    pub environment_loss: f64,
    /// `lastBoilRate`.
    pub boil_rate: i32,
    /// `lastMaxBoil`.
    pub max_boil: i32,
    /// Heated coolant cooled this tick.
    pub cooled: i64,
}

/// `BasicHeatCapacitor.update`: what was handed to it lands, unless it is too small to count (then it waits).
pub fn update(heat: &mut Heat, pending: &mut f64) {
    if *pending != 0.0 && pending.abs() > EPSILON {
        heat.stored += *pending;
        *pending = 0.0;
    }
}

/// Java's `Math.round(double)`.
fn java_round(v: f64) -> i64 {
    let r = (v + 0.5).floor();
    if r.is_nan() { 0 } else { r.clamp(i64::MIN as f64, i64::MAX as f64) as i64 }
}

/// `BoilerMultiblockData.tick` and the master's `updateHeatCapacitors` after it: the air takes
/// `(T - ambient) / 110001` degrees, what heat handlers handed it since lands (`pending`), the heated coolant cools
/// `round(conductivity * stored * (1 - T / coolant))` mB into the cooled tank (`cooled_takes`: empty or of the
/// coolant's cooled variant) for its enthalpy, then above 373.15 K water boils by the heat available, at most the
/// elements' and the steam tank's room; the heat the coolant gave and the boil used land at the end.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    tanks: &mut Tanks,
    caps: &Capacities,
    heat: &mut Heat,
    pending: &mut f64,
    ambient: f64,
    elements: i32,
    coolant: Option<HeatedCoolant>,
    cooled_takes: bool,
    config: &BoilerConfig,
) -> Tick {
    let mut out = Tick { hot: heat.temperature() >= BASE_BOIL_TEMP - 0.01, ..Tick::default() };
    // `simulateEnvironment`.
    let transfer = (heat.temperature() - ambient) / ENVIRONMENT_INVERSE_CONDUCTION;
    *pending -= transfer * heat.capacity;
    out.environment_loss = transfer.max(0.0);
    update(heat, pending);
    if tanks.heated > 0
        && let Some(c) = coolant
    {
        let portion = c.conductivity * tanks.heated as f64;
        let to_cool = java_round(portion * (1.0 - heat.temperature() / c.temperature));
        if to_cool > 0 && cooled_takes {
            let cooled = to_cool.min((caps.cooled - tanks.cooled).max(0));
            if cooled > 0 {
                tanks.cooled += cooled;
                *pending += cooled as f64 * c.thermal_enthalpy;
                tanks.heated = (tanks.heated - cooled).max(0);
                out.cooled = cooled;
            }
        }
    }
    let temperature = heat.temperature();
    if temperature >= BASE_BOIL_TEMP && tanks.water > 0 {
        let available = ((temperature - BASE_BOIL_TEMP) * (heat.capacity * config.water_conductivity)).min(config.superheating_heat_transfer * f64::from(elements));
        // `Mth.floor` of a double, into an int.
        out.max_boil = (STEAM_ENERGY_EFFICIENCY * available / config.water_thermal_enthalpy).floor() as i32;
        let room = (caps.steam - tanks.steam).clamp(0, i64::from(i32::MAX)) as i32;
        let boiled = out.max_boil.min(tanks.water).min(room).max(0);
        tanks.water -= boiled;
        tanks.steam += i64::from(boiled);
        *pending -= f64::from(boiled) * config.water_thermal_enthalpy / STEAM_ENERGY_EFFICIENCY;
        out.boil_rate = boiled;
    }
    update(heat, pending);
    out
}

/// `onCreated`: its heat capacity, 50 a wall block (`locations`), with the cache's heat coming in at its air.
pub fn heat_at_formation(cached: Option<Heat>, locations: usize, ambient: f64) -> Heat {
    Heat::resized(cached, CASING_HEAT_CAPACITY * locations as f64, ambient)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATM10: BoilerConfig = BoilerConfig {
        water_per_tank: 32_000,
        steam_per_tank: 320_000,
        heated_coolant_per_tank: 512_000,
        cooled_coolant_per_tank: 512_000,
        water_conductivity: 0.7,
        superheating_heat_transfer: 1.6e7,
        water_thermal_enthalpy: 10.0,
    };
    const SODIUM: HeatedCoolant = HeatedCoolant { temperature: 100_000.0, thermal_enthalpy: 5.0, conductivity: 0.4 };
    /// Plains (0.8f) as Mekanism reads it.
    const PLAINS: f64 = 300.0 + 25.0 * (0.8_f32 as f64 - 0.8);

    /// A boiler box of casings `size` (length, height, width) at the origin, and what is inside.
    fn boiler(size: [i32; 3], inner: &[(Pos, Inside)]) -> (BTreeMap<Pos, Node>, BTreeMap<Pos, Inside>) {
        let c = Cuboid::new([0, 0, 0], [size[0] - 1, size[1] - 1, size[2] - 1]);
        let nodes = c.positions().filter(|p| c.relative(*p) == multiblock::Relative::Walls).map(|p| (p, Node::Frame)).collect();
        (nodes, inner.iter().copied().collect())
    }

    fn layer(size: [i32; 3], y: i32, kind: Inside, skip: &[[i32; 2]]) -> Vec<(Pos, Inside)> {
        let mut out = Vec::new();
        for x in 1..size[0] - 1 {
            for z in 1..size[2] - 1 {
                if !skip.contains(&[x, z]) {
                    out.push(([x, y, z], kind));
                }
            }
        }
        out
    }

    fn try_form(size: [i32; 3], inner: &[(Pos, Inside)]) -> Result<Shape, Failure> {
        let (nodes, kinds) = boiler(size, inner);
        form(&nodes, |p| kinds.get(&p).copied().unwrap_or(Inside::Air)).map(|(_, s)| s)
    }

    #[test]
    fn the_oracle_s_formations() {
        let min = [(([1, 1, 1]), Inside::Element), ([1, 2, 1], Inside::Disperser)];
        // 3x5x3: 17 blocks of water (the bottom wall's 9, the 8 walls beside the element), 18 of steam (two layers).
        assert_eq!(try_form([3, 5, 3], &min), Ok(Shape { water_volume: 17, steam_volume: 18, elements: 1, disperser_y: 2 }));
        let mut full = layer([5, 5, 5], 1, Inside::Element, &[]);
        full.extend(layer([5, 5, 5], 2, Inside::Disperser, &[]));
        assert_eq!(try_form([5, 5, 5], &full).map(|s| (s.water_volume, s.steam_volume, s.elements)), Ok((41, 50, 9)));
        assert_eq!(try_form([3, 5, 3], &[([1, 1, 1], Inside::Element)]), Err(Failure::NoDisperser));
        assert_eq!(try_form([3, 5, 3], &[([1, 2, 1], Inside::Disperser)]), Err(Failure::InvalidSuperheating));
        let mut missing = layer([5, 5, 5], 1, Inside::Element, &[]);
        missing.extend(layer([5, 5, 5], 2, Inside::Disperser, &[[2, 2]]));
        assert_eq!(try_form([5, 5, 5], &missing), Err(Failure::MissingDisperser([2, 2, 2])));
        let mut extra = layer([5, 6, 5], 1, Inside::Element, &[[3, 3]]);
        extra.extend(layer([5, 6, 5], 3, Inside::Disperser, &[]));
        extra.push(([3, 1, 3], Inside::Disperser));
        assert_eq!(try_form([5, 6, 5], &extra), Err(Failure::ExtraDisperser));
        let above = [([1, 1, 1], Inside::Element), ([1, 2, 1], Inside::Disperser), ([1, 4, 1], Inside::Element)];
        assert_eq!(try_form([3, 6, 3], &above), Err(Failure::InvalidSuperheating));
        let mut split = vec![([1, 1, 1], Inside::Element), ([3, 1, 3], Inside::Element)];
        split.extend(layer([5, 5, 5], 2, Inside::Disperser, &[]));
        assert_eq!(try_form([5, 5, 5], &split), Err(Failure::InvalidSuperheating));
        // A pocket of air the walls don't reach still forms: the count leaves out the bottom wall the explore takes in.
        let mut pocket = layer([5, 6, 5], 1, Inside::Element, &[]);
        pocket.extend(layer([5, 6, 5], 2, Inside::Element, &[[2, 2]]));
        pocket.extend(layer([5, 6, 5], 3, Inside::Disperser, &[]));
        assert_eq!(try_form([5, 6, 5], &pocket).map(|s| (s.water_volume, s.elements)), Ok((57, 17)));
        assert_eq!(try_form([3, 4, 3], &min).map(|s| s.steam_volume), Ok(9), "the dispersers right under the top wall");
        let two = [([1, 1, 1], Inside::Element), ([1, 2, 1], Inside::Element), ([1, 3, 1], Inside::Disperser)];
        assert_eq!(try_form([3, 6, 3], &two).map(|s| (s.water_volume, s.steam_volume, s.elements)), Ok((25, 18, 2)));
        let mut dirt = min.to_vec();
        dirt.push(([1, 3, 1], Inside::Invalid));
        assert_eq!(try_form([3, 5, 3], &dirt), Err(Failure::InvalidInner([1, 3, 1])));
    }

    #[test]
    fn air_pockets_fail_once_they_outnumber_the_bottom_wall() {
        // 7x10x7: a full layer of elements, then six rings of them around 3x3 of air (54 pocketed, more than the
        // bottom wall's 49), the dispersers above.
        let size = [7, 10, 7];
        let mut inner = layer(size, 1, Inside::Element, &[]);
        let middle: Vec<[i32; 2]> = (2..5).flat_map(|x| (2..5).map(move |z| [x, z])).collect();
        for y in 2..8 {
            inner.extend(layer(size, y, Inside::Element, &middle));
        }
        inner.extend(layer(size, 8, Inside::Disperser, &[]));
        assert_eq!(try_form(size, &inner), Err(Failure::AirPockets));
        // Five rings (45 pocketed) still form.
        let mut five = layer(size, 1, Inside::Element, &[]);
        for y in 2..7 {
            five.extend(layer(size, y, Inside::Element, &middle));
        }
        five.extend(layer(size, 7, Inside::Disperser, &[]));
        assert!(try_form([7, 9, 7], &five).is_ok());
    }

    #[test]
    fn capacities_follow_the_volumes() {
        let shape = Shape { water_volume: 49, steam_volume: 50, elements: 1, disperser_y: 2 };
        assert_eq!(ATM10.capacities(&shape), Capacities { water: 1_568_000, steam: 16_000_000, heated: 25_088_000, cooled: 25_600_000 });
        assert_eq!(ATM10.boil_capacity(1), 320_000);
    }

    fn run_heater_boiler(ticks: usize) -> Vec<(f64, i64)> {
        use crate::heater::{Heater, HeaterConfig};
        use crate::heat::Ambient;
        // Boiler A of the oracle: 3x5x3 (42 wall blocks), water kept full, a heater of 10 000 J/t west of it.
        let config = HeaterConfig { efficiency: 0.6 };
        let mut heater = Heater { energy: 4_000_000, usage: 10_000, heat: None, pending: 0.0 };
        let air = Ambient::uniform(PLAINS);
        let mut heat = heat_at_formation(None, 42, PLAINS);
        let caps = ATM10.capacities(&Shape { water_volume: 17, steam_volume: 18, elements: 1, disperser_y: 2 });
        let mut tanks = Tanks { water: caps.water, ..Tanks::default() };
        let mut pending = 0.0;
        let mut steam = 0;
        let mut out = Vec::new();
        for _ in 0..ticks {
            tick(&mut tanks, &caps, &mut heat, &mut pending, PLAINS, 1, None, true, &ATM10);
            steam += tanks.steam;
            tanks.steam = 0;
            tanks.water = caps.water;
            let made = heater.tick(&config, &air, [None, None, None, None, None, Some(INVERSE_CONDUCTION)]);
            pending += made.handed[5];
            out.push((heat.temperature(), steam));
        }
        out
    }

    #[test]
    fn a_resistive_heater_warms_and_boils_it_as_on_the_oracle() {
        let rows = run_heater_boiler(200);
        // The oracle's temperatures at the end of ticks 3, 4, 10 and 40 after the heater came.
        let expect = [(3, 300.476_190_774_213_7), (4, 301.348_917_031_908_4), (10, 311.878_984_807_374_76), (40, 374.390_125_098_067_5)];
        for (t, want) in expect {
            let got = rows[t - 1].0;
            assert!((got - want).abs() < 1e-6, "tick {t}: {got} against {want}");
        }
        // Then about 120 mB a tick: 19 950 - 824 out of the valve from tick 40 to 200.
        let made = rows[199].1 - rows[39].1;
        assert_eq!(made, 19_950 - 824);
        assert!((rows[199].0 - 374.392_957_405_505_1).abs() < 1e-6);
    }

    #[test]
    fn heated_coolant_cools_into_its_tank_as_on_the_oracle() {
        // Boiler C: 5x5x5, nine elements, 1000 mB of superheated sodium a tick from a basic tank.
        let shape = Shape { water_volume: 41, steam_volume: 50, elements: 9, disperser_y: 2 };
        let caps = ATM10.capacities(&shape);
        let mut heat = heat_at_formation(None, 98, PLAINS);
        assert_eq!(heat.capacity, 4900.0);
        let mut tanks = Tanks { water: caps.water, heated: 1000, ..Tanks::default() };
        let mut pending = 0.0;
        let want = [(1601, 399, 300.407_143_155_166_07), (1963, 1037, 301.058_159_862_064_26), (2180, 1820, 301.857_129_834_356_04)];
        for (heated, cooled, t) in want {
            tick(&mut tanks, &caps, &mut heat, &mut pending, PLAINS, 9, Some(SODIUM), true, &ATM10);
            tanks.heated += 1000;
            assert_eq!((tanks.heated, tanks.cooled), (heated, cooled));
            assert!((heat.temperature() - t).abs() < 1e-9, "{} against {t}", heat.temperature());
        }
    }

    #[test]
    fn one_element_caps_the_boil_as_on_the_oracle() {
        // Boiler K: 5x5x5, one element, a creative tank of superheated sodium, water kept full, 64 000 mB of sodium
        // out of an output valve on the first tick.
        let shape = Shape { water_volume: 49, steam_volume: 50, elements: 1, disperser_y: 2 };
        let caps = ATM10.capacities(&shape);
        let mut heat = heat_at_formation(None, 98, PLAINS);
        let mut tanks = Tanks { water: caps.water, heated: caps.heated, ..Tanks::default() };
        let mut pending = 0.0;
        let made = tick(&mut tanks, &caps, &mut heat, &mut pending, PLAINS, 1, Some(SODIUM), true, &ATM10);
        tanks.cooled -= 64_000;
        tanks.heated = caps.heated;
        assert_eq!((made.cooled, made.boil_rate), (10_005_094, 0));
        assert_eq!(heat.stored, 51_495_470.001_460_314);
        let made = tick(&mut tanks, &caps, &mut heat, &mut pending, PLAINS, 1, Some(SODIUM), true, &ATM10);
        tanks.heated = caps.heated;
        assert!(made.hot);
        assert_eq!((made.boil_rate, made.max_boil, tanks.steam, tanks.cooled), (320_000, 320_000, 320_000, 18_921_676));
        assert!((heat.temperature() - 16_407.739_842_570_33).abs() < 1e-6, "{}", heat.temperature());
        tick(&mut tanks, &caps, &mut heat, &mut pending, PLAINS, 1, Some(SODIUM), true, &ATM10);
        assert_eq!(tanks.cooled, caps.cooled, "the cooled tank full: no more cooling");
        assert_eq!(heat.stored.round(), 97_788_828.0);
    }

    #[test]
    fn the_valve_modes_cycle() {
        assert_eq!(ValveMode::Input.next(), ValveMode::OutputSteam);
        assert_eq!(ValveMode::OutputCoolant.next(), ValveMode::Input);
        assert_eq!(ValveMode::from_name("output_steam"), Some(ValveMode::OutputSteam));
    }

    #[test]
    fn its_settings_come_from_general_toml() {
        let t: toml::Table = "[boiler]\nwaterPerTank = 32000\nsteamPerTank = 320000\nheatedCoolantPerTank = 512000\ncooledCoolantPerTank = 512000\nwaterConductivity = 0.7\nsuperheatingHeatTransfer = 1.6E7\n[energy_conversion]\nmaxEnergyPerSteam = 10\n".parse().unwrap();
        assert_eq!(BoilerConfig::from_general(Some(&t)), ATM10);
        assert_eq!(BoilerConfig::from_general(None), BoilerConfig::default());
    }
}
