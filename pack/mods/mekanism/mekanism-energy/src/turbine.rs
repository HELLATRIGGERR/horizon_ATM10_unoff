//! Mekanism Generators' Industrial Turbine (`TurbineValidator`, `TurbineMultiblockData`): a cuboid of turbine
//! casings, 5x3x5 to 17x18x17 with an odd length and width, with turbine valves, turbine vents and structural glass in
//! its walls. Inside, a column of turbine rotors (two blades each at most) rises from the bottom wall to the rotational
//! complex at the middle of a full layer of pressure dispersers; electromagnetic coils touch each other and the block
//! above the complex, saturating condensers sit anywhere above the dispersers, vents no lower than them. Its steam tank
//! holds `chemicalPerTank` a block of the lower volume (the rotors' height times its length and width), its energy
//! buffer `energyCapacityPerVolume` a block of the whole, its vent tank `condenserRate` a condenser. A tick, steam
//! flows at most `disperserChemicalFlow` a disperser a block of the lower volume and `ventChemicalFlow` a vent, scaled
//! by how full the tank is and held to the energy room, and turns into `maxEnergyPerSteam / 28` Joules a mB for each
//! blade the coils carry (`bladesPerCoil` a coil); the condensers turn it back into water. Its dump modes
//! (`GasMode`) let steam go: all of it at `max(stored / 50, last input * 2)` a tick, or what lies above
//! `dumpExcessKeepRatio` of the tank.
use crate::boiler::explore_count;
use crate::multiblock::{self, Failure, Formation, Inner, Node, Pos};
use crate::separator::GasMode;
use crate::sides::Direction;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// `new VoxelCuboid(5, 3, 5)`, `new VoxelCuboid(17, 18, 17)`: (length, height, width).
pub const MIN_BOUNDS: [i32; 3] = [5, 3, 5];
pub const MAX_BOUNDS: [i32; 3] = [17, 18, 17];
/// `TurbineValidator.MAX_BLADES`: the blades `maxEnergyPerSteam` is shared among.
pub const MAX_BLADES: f64 = 28.0;
/// `TurbineMultiblockData.ROTATION_THRESHOLD`.
pub const ROTATION_THRESHOLD: f32 = 0.001;

/// `generators.toml` `[turbine]`, `general.toml` `[energy_conversion] maxEnergyPerSteam` and
/// `[auto_eject] dumpExcessKeepRatio`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurbineConfig {
    pub blades_per_coil: i32,
    pub vent_chemical_flow: f64,
    pub disperser_chemical_flow: f64,
    pub condenser_rate: i32,
    pub energy_capacity_per_volume: i64,
    pub chemical_per_tank: i64,
    pub max_energy_per_steam: f64,
    pub dump_excess_keep_ratio: f64,
}

impl Default for TurbineConfig {
    /// Mekanism's defaults (ATM10: 8 blades a coil, 43 478.262 mB a vent, 128 000 a condenser, 6400 a block).
    fn default() -> Self {
        Self {
            blades_per_coil: 4,
            vent_chemical_flow: 32_000.0,
            disperser_chemical_flow: 1280.0,
            condenser_rate: 64_000,
            energy_capacity_per_volume: 16_000_000,
            chemical_per_tank: 64_000,
            max_energy_per_steam: 10.0,
            dump_excess_keep_ratio: 0.85,
        }
    }
}

impl TurbineConfig {
    pub fn from_tables(generators: Option<&toml::Table>, general: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let float = |t: &toml::Table, key: &str, lo: f64, hi: f64| {
            t.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).filter(|v| (lo..=hi).contains(v))
        };
        let long = |t: &toml::Table, key: &str, lo: i64, hi: i64| t.get(key).and_then(toml::Value::as_integer).filter(|v| (lo..=hi).contains(v));
        if let Some(t) = generators.and_then(|g| g.get("turbine")).and_then(toml::Value::as_table) {
            if let Some(v) = long(t, "bladesPerCoil", 1, 12) {
                c.blades_per_coil = v as i32;
            }
            if let Some(v) = float(t, "ventChemicalFlow", 0.1, 1_024_000.0) {
                c.vent_chemical_flow = v;
            }
            if let Some(v) = float(t, "disperserChemicalFlow", 0.1, 1_024_000.0) {
                c.disperser_chemical_flow = v;
            }
            if let Some(v) = long(t, "condenserRate", 1, 2_000_000) {
                c.condenser_rate = v as i32;
            }
            if let Some(v) = long(t, "energyCapacityPerVolume", 1, 1_000_000_000_000) {
                c.energy_capacity_per_volume = v;
            }
            // `Long.MAX_VALUE / maxTurbine` (17 * 17 * 18).
            if let Some(v) = long(t, "chemicalPerTank", 1, i64::MAX / 5202) {
                c.chemical_per_tank = v;
            }
        }
        if let Some(g) = general {
            if let Some(v) = g.get("energy_conversion").and_then(toml::Value::as_table).and_then(|s| s.get("maxEnergyPerSteam")).and_then(toml::Value::as_integer).filter(|v| *v >= 0) {
                c.max_energy_per_steam = v as f64;
            }
            if let Some(v) = g.get("auto_eject").and_then(toml::Value::as_table).and_then(|s| float(s, "dumpExcessKeepRatio", 0.001, 1.0)) {
                c.dump_excess_keep_ratio = v;
            }
        }
        c
    }

    /// `<config>/Mekanism/generators.toml` and `general.toml`.
    pub fn load(config: &Path) -> Self {
        let read = |name: &str| std::fs::read_to_string(config.join("Mekanism").join(name)).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tables(read("generators.toml").as_ref(), read("general.toml").as_ref())
    }

    /// `getSteamCapacity`: `lowerVolume * chemicalPerTank`.
    pub fn steam_capacity(&self, shape: &Shape) -> i64 {
        i64::from(shape.lower_volume.max(0)).saturating_mul(self.chemical_per_tank)
    }

    /// `setVolume`: `volume * energyCapacityPerVolume`.
    pub fn energy_capacity(&self, shape: &Shape) -> u64 {
        (i64::from(shape.volume.max(0)).saturating_mul(self.energy_capacity_per_volume)).max(0) as u64
    }

    /// The vent tank's capacity once formed: `condensers * condenserRate`.
    pub fn water_capacity(&self, shape: &Shape) -> i32 {
        shape.condensers.max(0).saturating_mul(self.condenser_rate)
    }

    /// The Joules a mB of steam makes: `maxEnergyPerSteam / 28 * min(blades, coils * bladesPerCoil)`.
    pub fn multiplier(&self, blades: i32, coils: i32) -> f64 {
        self.max_energy_per_steam / MAX_BLADES * f64::from(blades.min(coils.saturating_mul(self.blades_per_coil)))
    }

    /// `getMaxFlowRate` before its clamp: the dispersers' flow over the lower volume, at most the vents'.
    pub fn max_rate(&self, shape: &Shape) -> f64 {
        let rate = f64::from(shape.lower_volume) * (f64::from(shape.dispersers()) * self.disperser_chemical_flow);
        rate.min(f64::from(shape.vents()) * self.vent_chemical_flow)
    }

    /// `getMaxFlowRate`.
    pub fn max_flow(&self, shape: &Shape) -> i64 {
        crate::clamp_to_long(self.max_rate(shape))
    }

    /// `getMaxProduction`.
    pub fn max_production(&self, shape: &Shape, blades: i32) -> i64 {
        crate::clamp_to_long(self.multiplier(blades, shape.coils) * self.max_rate(shape))
    }

    /// `getProductionRate` at a flow.
    pub fn production(&self, flow: i64, blades: i32, coils: i32) -> i64 {
        crate::clamp_to_long(self.multiplier(blades, coils) * flow as f64)
    }

    /// `getMaxWaterOutput`.
    pub fn max_water_output(&self, shape: &Shape) -> i64 {
        i64::from(shape.condensers) * i64::from(self.condenser_rate)
    }
}

// ---- Formation ----------------------------------------------------------------------------------------------

/// What the validator finds inside a turbine at a position (`validateInner`: air, or one of its inner blocks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inside {
    Air,
    Disperser,
    /// A turbine rotor and the blades it houses.
    Rotor(i32),
    Complex,
    Coil,
    Condenser,
    Invalid,
    Unloaded,
}

/// What `postcheck` works out of a formed turbine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub length: i32,
    pub width: i32,
    /// The whole box (`getVolume`).
    pub volume: i32,
    /// `lowerVolume`: length * width * the rotors' height.
    pub lower_volume: i32,
    /// The rotors under the complex.
    pub rotors: i32,
    /// The blades they house as it formed.
    pub blades: i32,
    pub coils: i32,
    pub condensers: i32,
    /// `complex`: the rotational complex.
    pub complex: Pos,
    /// `ventData`: each vent and the side it looks out of.
    pub vents: Vec<(Pos, Direction)>,
}

impl Shape {
    /// `getDispersers`: the layer around the complex.
    pub fn dispersers(&self) -> i32 {
        (self.length - 2) * (self.width - 2) - 1
    }

    pub fn vents(&self) -> i32 {
        self.vents.len() as i32
    }
}

/// `FormationProtocol.doUpdate` with the `TurbineValidator`: the cuboid (no message when there is none), every block
/// of it (vents are its `OTHER` blocks; inside, dispersers, rotors, the complex, coils and condensers besides air), then
/// `postcheck`.
pub fn form(nodes: &BTreeMap<Pos, Node>, mut inside: impl FnMut(Pos) -> Inside) -> Result<(Formation, Shape), Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = multiblock::fetch_cuboid(nodes, MIN_BOUNDS, MAX_BOUNDS).ok_or(Failure::Fail)?;
    let mut kinds: BTreeMap<Pos, Inside> = BTreeMap::new();
    let formation = multiblock::validate(cuboid, nodes, |p| {
        let kind = inside(p);
        kinds.insert(p, kind);
        match kind {
            Inside::Air => Inner::Air,
            Inside::Invalid => Inner::Invalid,
            Inside::Unloaded => Inner::Unloaded,
            _ => Inner::Internal,
        }
    })?;
    let vents: Vec<Pos> = formation.locations.iter().copied().filter(|p| nodes.get(p) == Some(&Node::Other)).collect();
    let shape = postcheck(&formation, &kinds, &vents)?;
    Ok((formation, shape))
}

/// `TurbineValidator.postcheck` over the inside `kinds` of a validated structure and its vents.
fn postcheck(formation: &Formation, kinds: &BTreeMap<Pos, Inside>, vents: &[Pos]) -> Result<Shape, Failure> {
    let cuboid = formation.cuboid;
    let (length, width) = (cuboid.length(), cuboid.width());
    if length % 2 != 1 || width % 2 != 1 {
        return Err(Failure::EvenLength);
    }
    let min = cuboid.min;
    let center_x = min[0] + (length - 1) / 2;
    let center_z = min[2] + (width - 1) / 2;
    let kind = |p: Pos| kinds.get(&p).copied();
    let mut complex = None;
    let mut turbines: BTreeSet<Pos> = BTreeSet::new();
    let mut dispersers: BTreeSet<Pos> = BTreeSet::new();
    let mut coils: Vec<Pos> = Vec::new();
    let mut condensers: Vec<Pos> = Vec::new();
    for &p in &formation.internal {
        match kind(p) {
            Some(Inside::Complex) => {
                if complex.is_some() || p[0] != center_x || p[2] != center_z {
                    return Err(Failure::BadComplex(p));
                }
                complex = Some(p);
            }
            Some(Inside::Rotor(_)) => {
                if p[0] != center_x || p[2] != center_z {
                    return Err(Failure::BadRotor(p));
                }
                turbines.insert(p);
            }
            Some(Inside::Disperser) => {
                dispersers.insert(p);
            }
            Some(Inside::Coil) => coils.push(p),
            Some(Inside::Condenser) => condensers.push(p),
            _ => {}
        }
    }
    let complex = complex.ok_or(Failure::MissingComplex)?;
    let rotors = complex[1] - min[1] + 1;
    let inner_radius = (length.min(width) - 3) / 2;
    if inner_radius < rotors / 4 {
        return Err(Failure::TooNarrow);
    }
    if coils.is_empty() {
        return Err(Failure::MissingCoils);
    }
    if dispersers.is_empty() {
        return Err(Failure::MissingDispersers);
    }
    let (radius_x, radius_z) = ((length - 3) / 2, (width - 3) / 2);
    for x in complex[0] - radius_x..=complex[0] + radius_x {
        for z in complex[2] - radius_z..=complex[2] + radius_z {
            if x == center_x && z == center_z {
                continue;
            }
            let p = [x, complex[1], z];
            if kind(p) != Some(Inside::Disperser) {
                return Err(Failure::MissingDisperser(p));
            }
            dispersers.remove(&p);
        }
    }
    if !dispersers.is_empty() {
        return Err(Failure::MalformedDispersers);
    }
    if let Some(&p) = condensers.iter().find(|p| p[1] <= complex[1]) {
        return Err(Failure::CondenserBelowComplex(p));
    }
    let mut height = 0;
    let mut blades = 0;
    for y in (min[1] + 1..complex[1]).rev() {
        let p = [center_x, y, center_z];
        let Some(Inside::Rotor(n)) = kind(p) else { return Err(Failure::RotorsNotContiguous) };
        height += 1;
        blades += n;
        turbines.remove(&p);
    }
    if !turbines.is_empty() {
        return Err(Failure::BadRotors);
    }
    if blades == 0 {
        return Err(Failure::NoBlades);
    }
    let above = [complex[0], complex[1] + 1, complex[2]];
    let connected = explore_count(above, |p| kind(p) == Some(Inside::Coil));
    if coils.len() as i32 > connected {
        return Err(Failure::MalformedCoils);
    }
    let mut vent_data = Vec::new();
    for &p in vents {
        if p[1] < complex[1] {
            return Err(Failure::VentBelowComplex(p));
        }
        if let Some(side) = cuboid.side_of(p) {
            vent_data.push((p, side));
        }
    }
    if vent_data.is_empty() {
        return Err(Failure::MissingVents);
    }
    Ok(Shape {
        length,
        width,
        volume: cuboid.volume(),
        lower_volume: length * width * height,
        rotors: height,
        blades,
        coils: connected,
        condensers: condensers.len() as i32,
        complex,
        vents: vent_data,
    })
}

// ---- The tick ------------------------------------------------------------------------------------------------

/// A turbine's contents: steam (mB), energy (Joules), water in its vent tank (mB).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    pub steam: i64,
    pub energy: u64,
    pub water: i32,
}

/// What a tick did: `clientFlow` (the mB of steam that flowed), the Joules and water it made and the rotors' speed
/// (`flowRate`, the share of the most it could flow).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    pub flow: i64,
    pub energy: i64,
    pub water: i32,
    pub rotation: f64,
}

/// `MathUtils.clampToInt(double)`.
fn clamp_to_int(d: f64) -> i32 {
    if d.is_nan() {
        0
    } else if d < f64::from(i32::MAX) {
        d as i32
    } else {
        i32::MAX
    }
}

/// The steam's flow in `TurbineMultiblockData.tick`: with steam and room for energy, `rate` (the most it flows) times
/// how full its tank is, at most the steam and what the energy room takes, turns into Joules and, as far as the
/// condensers reach and the vent tank has room, water.
pub fn produce(contents: &mut Contents, shape: &Shape, blades: i32, config: &TurbineConfig) -> Tick {
    let energy_capacity = config.energy_capacity(shape);
    let needed = energy_capacity.saturating_sub(contents.energy);
    let stored = contents.steam;
    if stored <= 0 || needed == 0 {
        return Tick::default();
    }
    let multiplier = config.multiplier(blades, shape.coils);
    if multiplier < f64::from(1.0e-5_f32) {
        return Tick::default();
    }
    let rate = config.max_rate(shape);
    let capacity = config.steam_capacity(shape);
    let proportion = stored as f64 / capacity as f64;
    let flow = (stored as f64).min(rate).min(needed as f64 / multiplier) * proportion;
    let client = crate::clamp_to_long(flow);
    if client <= 0 {
        return Tick { flow: client.max(0), ..Tick::default() };
    }
    let made = crate::clamp_to_long(multiplier * flow).max(0) as u64;
    let energy = made.min(needed);
    contents.energy += energy;
    contents.steam = (contents.steam - client).max(0);
    let condensed = clamp_to_int(flow).min(config.water_capacity(shape)).max(0);
    let water = condensed.min(config.water_capacity(shape).saturating_sub(contents.water).max(0));
    contents.water += water;
    Tick { flow: client, energy: energy as i64, water, rotation: flow / rate }
}

/// `getDumpingAmount`.
fn dumping_amount(stored: i64, last_input: i64) -> i64 {
    stored.min((stored / 50).max(last_input.saturating_mul(2)))
}

/// The dump at the end of the tick: what `mode` lets go of the steam, with what came in the tick before
/// (`lastSteamInput`). What went.
pub fn dump(contents: &mut Contents, shape: &Shape, mode: GasMode, last_input: i64, config: &TurbineConfig) -> i64 {
    let amount = contents.steam;
    if amount <= 0 {
        return 0;
    }
    let gone = match mode {
        GasMode::Idle => 0,
        GasMode::Dumping => dumping_amount(amount, last_input),
        GasMode::DumpingExcess => {
            let target = crate::clamp_to_long(config.steam_capacity(shape) as f64 * config.dump_excess_keep_ratio);
            if target < amount { (amount - target).min(dumping_amount(amount, last_input)) } else { 0 }
        }
    };
    contents.steam -= gone;
    gone
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiblock::Node;

    fn atm10() -> TurbineConfig {
        let g: toml::Table = "[turbine]\nbladesPerCoil = 8\nventChemicalFlow = 43478.262\ndisperserChemicalFlow = 1280.0\ncondenserRate = 128000\nenergyCapacityPerVolume = 16000000\nchemicalPerTank = 6400\n".parse().unwrap();
        let general: toml::Table = "[auto_eject]\ndumpExcessKeepRatio = 0.85\n[energy_conversion]\nmaxEnergyPerSteam = 10\n".parse().unwrap();
        TurbineConfig::from_tables(Some(&g), Some(&general))
    }

    /// A turbine of `size` at the origin: casings on the walls, `walls` over them, `inner` inside (the rest air).
    fn build(size: [i32; 3], walls: &[(Pos, Node)], inner: &[(Pos, Inside)]) -> Result<(Formation, Shape), Failure> {
        let mut nodes = BTreeMap::new();
        for x in 0..size[0] {
            for y in 0..size[1] {
                for z in 0..size[2] {
                    let inside = x > 0 && y > 0 && z > 0 && x < size[0] - 1 && y < size[1] - 1 && z < size[2] - 1;
                    if !inside {
                        nodes.insert([x, y, z], Node::Frame);
                    }
                }
            }
        }
        for (p, n) in walls {
            nodes.insert(*p, *n);
        }
        let inner: BTreeMap<Pos, Inside> = inner.iter().copied().collect();
        form(&nodes, |p| inner.get(&p).copied().unwrap_or(Inside::Air))
    }

    /// The 5-wide turbine of `rotors` rotors: the complex and the ring above them, a coil above the complex.
    fn base(rotors: i32, blades: i32) -> Vec<(Pos, Inside)> {
        let cy = rotors + 1;
        let mut v: Vec<(Pos, Inside)> = (1..cy).map(|y| ([2, y, 2], Inside::Rotor(if y == 1 { blades } else { 0 }))).collect();
        v.push(([2, cy, 2], Inside::Complex));
        for x in 1..4 {
            for z in 1..4 {
                if (x, z) != (2, 2) {
                    v.push(([x, cy, z], Inside::Disperser));
                }
            }
        }
        v.push(([2, cy + 1, 2], Inside::Coil));
        v
    }

    fn with(mut v: Vec<(Pos, Inside)>, extra: &[(Pos, Inside)]) -> Vec<(Pos, Inside)> {
        for (p, k) in extra {
            v.retain(|(q, _)| q != p);
            v.push((*p, *k));
        }
        v
    }

    #[test]
    fn forms_as_the_oracle_saw() {
        let vent = [([2, 3, 0], Node::Other)];
        let ok = build([5, 5, 5], &vent, &base(1, 1)).unwrap().1;
        assert_eq!((ok.lower_volume, ok.rotors, ok.blades, ok.coils, ok.condensers, ok.dispersers(), ok.volume), (25, 1, 1, 1, 0, 8, 125));
        assert_eq!(ok.vents, vec![([2, 3, 0], Direction::North)]);
        assert_eq!(build([5, 5, 5], &vent, &base(1, 0)).unwrap_err(), Failure::NoBlades);
        assert_eq!(build([6, 5, 5], &vent, &base(1, 1)).unwrap_err(), Failure::EvenLength);
        assert_eq!(build([5, 5, 5], &vent, &with(base(1, 1), &[([2, 2, 2], Inside::Disperser)])).unwrap_err(), Failure::MissingComplex);
        assert_eq!(build([5, 5, 5], &vent, &with(base(1, 1), &[([2, 2, 2], Inside::Disperser), ([1, 2, 1], Inside::Complex)])).unwrap_err(), Failure::BadComplex([1, 2, 1]));
        let no_coil: Vec<_> = base(1, 1).into_iter().filter(|(_, k)| *k != Inside::Coil).collect();
        assert_eq!(build([5, 5, 5], &vent, &no_coil).unwrap_err(), Failure::MissingCoils);
        assert_eq!(build([5, 5, 5], &[], &base(1, 1)).unwrap_err(), Failure::MissingVents);
        assert_eq!(build([5, 5, 5], &[([2, 1, 0], Node::Other)], &base(1, 1)).unwrap_err(), Failure::VentBelowComplex([2, 1, 0]));
        assert!(build([5, 5, 5], &[([2, 2, 0], Node::Other)], &base(1, 1)).is_ok(), "a vent at the complex's height");
        assert_eq!(build([5, 5, 5], &vent, &with(base(1, 1), &[([1, 1, 1], Inside::Condenser)])).unwrap_err(), Failure::CondenserBelowComplex([1, 1, 1]));
        let gap = with(base(2, 1), &[([2, 2, 2], Inside::Air)]);
        assert_eq!(build([5, 6, 5], &[([2, 4, 0], Node::Other)], &gap).unwrap_err(), Failure::RotorsNotContiguous);
        assert_eq!(build([5, 5, 5], &vent, &with(base(1, 1), &[([1, 1, 1], Inside::Rotor(0))])).unwrap_err(), Failure::BadRotor([1, 1, 1]));
        let missing: Vec<_> = base(1, 1).into_iter().filter(|(p, _)| *p != [1, 2, 1]).collect();
        assert_eq!(build([5, 5, 5], &vent, &missing).unwrap_err(), Failure::MissingDisperser([1, 2, 1]));
        assert_eq!(build([5, 5, 5], &vent, &with(base(1, 1), &[([1, 3, 1], Inside::Disperser)])).unwrap_err(), Failure::MalformedDispersers);
        assert_eq!(build([5, 5, 5], &vent, &with(base(1, 1), &[([1, 3, 1], Inside::Coil)])).unwrap_err(), Failure::MalformedCoils);
        let five = build([5, 9, 5], &[([2, 7, 0], Node::Other)], &base(5, 1)).unwrap().1;
        assert_eq!((five.lower_volume, five.rotors), (125, 5));
        assert_eq!(build([5, 10, 5], &[([2, 8, 0], Node::Other)], &base(6, 1)).unwrap_err(), Failure::TooNarrow);
        assert!(matches!(build([5, 5, 5], &[([2, 3, 0], Node::Other), ([0, 3, 0], Node::Other)], &base(1, 1)), Err(Failure::InvalidFrame(_))));
        assert!(matches!(build([5, 5, 5], &vent, &with(base(1, 1), &[([1, 3, 1], Inside::Invalid)])), Err(Failure::InvalidInner(_))));
        let mixed = build([5, 5, 5], &vent, &with(base(1, 1), &[([1, 3, 1], Inside::Condenser), ([3, 3, 3], Inside::Condenser), ([1, 3, 3], Inside::Condenser)])).unwrap().1;
        assert_eq!(mixed.condensers, 3);
        let top = build([5, 5, 5], &[([2, 4, 1], Node::Other)], &base(1, 1)).unwrap().1;
        assert_eq!(top.vents, vec![([2, 4, 1], Direction::Up)]);
    }

    fn shape(size: [i32; 3], rotors: i32, coils: i32, condensers: i32, vents: usize) -> Shape {
        Shape {
            length: size[0],
            width: size[2],
            volume: size[0] * size[1] * size[2],
            lower_volume: size[0] * size[2] * rotors,
            rotors,
            blades: 0,
            coils,
            condensers,
            complex: [0; 3],
            vents: vec![([0; 3], Direction::North); vents],
        }
    }

    #[test]
    fn rates_as_the_oracle_measured() {
        let c = atm10();
        // A: 5x5x5, two blades, one coil, one condenser, one vent, full of steam.
        let a = shape([5, 5, 5], 1, 1, 1, 1);
        assert_eq!((c.steam_capacity(&a), c.energy_capacity(&a), c.water_capacity(&a)), (160_000, 2_000_000_000, 128_000));
        let mut t = Contents { steam: 160_000, ..Contents::default() };
        let tick = produce(&mut t, &a, 2, &c);
        assert_eq!((tick.flow, tick.energy, tick.water, t.steam), (43_478, 31_055, 43_478, 116_522));
        // C: 7x7x7, two rotors of four blades, two coils, two condensers, two vents.
        let cc = shape([7, 7, 7], 2, 2, 2, 2);
        let mut t = Contents { steam: 627_200, ..Contents::default() };
        let tick = produce(&mut t, &cc, 4, &c);
        assert_eq!((tick.flow, tick.energy, tick.water), (86_956, 124_223, 86_956));
        // D: 5x9x5, ten blades on one coil (eight carried).
        let d = shape([5, 9, 5], 5, 1, 0, 1);
        let mut t = Contents { steam: 800_000, ..Contents::default() };
        let tick = produce(&mut t, &d, 10, &c);
        assert_eq!((tick.flow, tick.energy, tick.water), (43_478, 124_223, 0));
        // B: 1000 mB a tick from a basic tank: 6 flowed of the first 1000, 4 J.
        let b = shape([5, 5, 5], 1, 1, 0, 1);
        let mut t = Contents { steam: 1000, ..Contents::default() };
        let tick = produce(&mut t, &b, 2, &c);
        assert_eq!((tick.flow, tick.energy, t.steam), (6, 4, 994));
        let mut steam = 994;
        let mut energy = 4;
        for _ in 3..=8 {
            let mut t = Contents { steam: steam + 1000, energy, water: 0 };
            produce(&mut t, &b, 2, &c);
            steam = t.steam;
            energy = t.energy;
        }
        assert_eq!((steam + 1000, energy), (7213, 561), "B on the eighth tick");
    }

    #[test]
    fn dumps() {
        let c = atm10();
        let a = shape([5, 5, 5], 1, 1, 0, 1);
        let mut t = Contents { steam: 116_522, ..Contents::default() };
        assert_eq!(dump(&mut t, &a, GasMode::Dumping, 0, &c), 2330);
        let mut t = Contents { steam: 1500, ..Contents::default() };
        assert_eq!(dump(&mut t, &a, GasMode::Dumping, 1000, &c), 1500);
        let q = shape([5, 9, 5], 5, 1, 0, 1);
        let mut t = Contents { steam: 756_522, ..Contents::default() };
        assert_eq!(dump(&mut t, &q, GasMode::DumpingExcess, 0, &c), 15_130);
        let mut t = Contents { steam: 756_522, ..Contents::default() };
        assert_eq!(dump(&mut t, &q, GasMode::DumpingExcess, 43_478, &c), 76_522);
        let mut t = Contents { steam: 600_000, ..Contents::default() };
        assert_eq!(dump(&mut t, &q, GasMode::DumpingExcess, 43_478, &c), 0);
        // The oracle's turbines (local `mekturb-work/t2.py`), their dump modes and steam set in the cache, nothing fed:
        // steam and Joules after each of their first two ticks.
        for (s, blades, mode, steam, want) in [
            (a.clone(), 2, GasMode::Dumping, 160_000, [(114_192, 31_055), (81_499, 53_219)]),
            (q.clone(), 10, GasMode::DumpingExcess, 800_000, [(741_392, 124_223), (687_078, 239_345)]),
            (a.clone(), 2, GasMode::Idle, 160_000, [(116_522, 31_055), (84_859, 53_671)]),
        ] {
            let mut t = Contents { steam, ..Contents::default() };
            for (i, w) in want.iter().enumerate() {
                produce(&mut t, &s, blades, &c);
                dump(&mut t, &s, mode, 0, &c);
                assert_eq!((t.steam, t.energy), (w.0, w.1 as u64), "{mode:?} tick {}", i + 1);
            }
        }
        // Filled in one tick from a tank (the world test's P and Q): the next tick's dump sees all of it as the last
        // input, the one after none.
        for (s, blades, mode, steam, want) in [
            (a.clone(), 2, GasMode::Dumping, 160_000, [(0, 31_055), (0, 31_055)]),
            (q.clone(), 10, GasMode::DumpingExcess, 800_000, [(680_000, 124_223), (643_044, 229_813)]),
        ] {
            let mut t = Contents { steam, ..Contents::default() };
            for (i, w) in want.iter().enumerate() {
                produce(&mut t, &s, blades, &c);
                dump(&mut t, &s, mode, if i == 0 { steam } else { 0 }, &c);
                assert_eq!((t.steam, t.energy), (w.0, w.1 as u64), "{mode:?} filled, tick {}", i + 1);
            }
        }
    }
}
