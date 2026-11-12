//! Mekanism's Supercritical Phase Shifter (`SPSValidator`, `SPSMultiblockData`, `SPSCache`): a 7x7x7 of SPS casings
//! whose faces leave their corners out (`ALLOWED_GRID`: `fetchCuboid` over every side, missing at most 72 positions),
//! SPS ports and structural or reactor glass in the faces' middles, and inside only air and supercharged coils, each
//! right behind a port (`postcheck`). Its input tank takes polonium (twice `inputPerAntimatter` mB), its output tank
//! holds `outputTankCapacity` mB of antimatter. A port with a coil behind it hands the multiblock all its Joules each
//! tick while it can operate; the multiblock processes `energy / energyPerInput` mB of polonium a tick (the fraction
//! carried in `progress`), each `inputPerAntimatter` mB making one mB of antimatter (`general.toml` `[sps]`; the
//! ports' store is `machine-storage.toml` `spsPort`).
use crate::multiblock::{self, Cuboid, Failure, Formation, Inner, Node, Pos, Requirement, Side};
use crate::sides::Direction;
use std::collections::BTreeMap;
use std::path::Path;

/// `SPSValidator.BOUNDS`: exactly 7 each way.
pub const SIZE: [i32; 3] = [7, 7, 7];
/// `precheck`'s tolerance: twelve positions left out of each face.
pub const TOLERANCE: i32 = 72;
/// `ALLOWED_GRID`, by the face's horizontal then vertical coordinate: 0 ignored, 1 a casing (`FRAME`), 2 any block of
/// the structure (`OTHER`).
pub const ALLOWED_GRID: [[u8; 7]; 7] = [
    [0, 0, 1, 1, 1, 0, 0],
    [0, 1, 2, 2, 2, 1, 0],
    [1, 2, 2, 2, 2, 2, 1],
    [1, 2, 2, 2, 2, 2, 1],
    [1, 2, 2, 2, 2, 2, 1],
    [0, 1, 2, 2, 2, 1, 0],
    [0, 0, 1, 1, 1, 0, 0],
];

/// `general.toml` `[sps]` and `machine-storage.toml` `spsPort`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpsConfig {
    pub input_per_antimatter: i32,
    pub output_tank_capacity: i64,
    pub energy_per_input: i64,
    pub port_storage: u64,
}

impl Default for SpsConfig {
    /// Mekanism's defaults (ATM10 sets `energyPerInput` 100 000).
    fn default() -> Self {
        Self { input_per_antimatter: 1000, output_tank_capacity: 1000, energy_per_input: 1_000_000, port_storage: 1_000_000_000 }
    }
}

impl SpsConfig {
    pub fn from_tables(general: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        if let Some(s) = general.and_then(|g| g.get("sps")).and_then(toml::Value::as_table) {
            let int = |key: &str| s.get(key).and_then(toml::Value::as_integer);
            if let Some(v) = int("inputPerAntimatter").filter(|v| (1..=i64::from(i32::MAX)).contains(v)) {
                c.input_per_antimatter = v as i32;
            }
            if let Some(v) = int("outputTankCapacity").filter(|v| *v >= 1) {
                c.output_tank_capacity = v;
            }
            if let Some(v) = int("energyPerInput").filter(|v| *v >= 0) {
                c.energy_per_input = v;
            }
        }
        if let Some(v) = storage.and_then(|t| t.get("spsPort")).and_then(toml::Value::as_integer).filter(|v| *v >= 1) {
            c.port_storage = v as u64;
        }
        c
    }

    /// `<config>/Mekanism/general.toml` and `machine-storage.toml`.
    pub fn load(config: &Path) -> Self {
        let read = |name: &str| std::fs::read_to_string(config.join("Mekanism").join(name)).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tables(read("general.toml").as_ref(), read("machine-storage.toml").as_ref())
    }

    /// `getMaxInputGas`: the input tank's mB.
    pub fn input_capacity(&self) -> i64 {
        i64::from(self.input_per_antimatter) * 2
    }
}

// ---- Formation ----------------------------------------------------------------------------------------------

/// `SPSValidator.getStructureRequirement`: a wall position by `ALLOWED_GRID` over the face it lies on (any face of an
/// edge reads the same, the grid being symmetric), the inside the cuboid's own.
pub fn requirement(cuboid: &Cuboid, p: Pos) -> Requirement {
    let d = [p[0] - cuboid.min[0], p[1] - cuboid.min[1], p[2] - cuboid.min[2]];
    let size = cuboid.size();
    let Some(axis) = (0..3).find(|&a| d[a] == 0 || d[a] == size[a] - 1) else { return cuboid.requirement(p) };
    // `Structure.Axis.horizontal` and `vertical`.
    let (h, v) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let at = |c: i32| usize::try_from(c).ok().filter(|c| *c < 7);
    match (at(d[h]), at(d[v])) {
        (Some(h), Some(v)) => match ALLOWED_GRID[h][v] {
            0 => Requirement::Ignored,
            1 => Requirement::Frame,
            _ => Requirement::Other,
        },
        _ => Requirement::Frame,
    }
}

/// `FormationProtocol.doUpdate` with the `SPSValidator`: the cuboid over every side within `TOLERANCE`, every block
/// by `requirement` (`inner` says what is inside: air or a supercharged coil passes), then `postcheck`: every block
/// inside must be a coil right behind a port.
pub fn form(nodes: &BTreeMap<Pos, Node>, inner: impl FnMut(Pos) -> Inner) -> Result<Formation, Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = multiblock::fetch_cuboid_sides(nodes, SIZE, SIZE, &Side::ALL, TOLERANCE).ok_or(Failure::Fail)?;
    let formation = multiblock::validate_with(cuboid, nodes, |p| requirement(&cuboid, p), |_, _| None, inner)?;
    if coils(&formation.valves, &formation.internal).len() != formation.internal.len() {
        return Err(Failure::DisconnectedCoil);
    }
    Ok(formation)
}

/// `postcheck`'s `addCoil`: the ports with a block of the inside right behind them, each with that block.
pub fn coils(valves: &[(Pos, Direction)], internal: &[Pos]) -> Vec<(Pos, Pos)> {
    let mut out: Vec<(Pos, Pos)> = Vec::new();
    for (p, side) in valves {
        let o = side.opposite().offset();
        let behind = [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
        if internal.contains(&behind) && !out.iter().any(|(_, c)| *c == behind) {
            out.push((*p, behind));
        }
    }
    out
}

// ---- The process ----------------------------------------------------------------------------------------------

/// `SPSMultiblockData`'s own state (`SPSCache`: `progress`, `processed`, `could_operate`, `energy_usage` (the Joules
/// received for its next tick), `last_processed`); `last_received` (`lastReceivedEnergy`, for its screen) isn't saved.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Core {
    pub progress: f64,
    pub input_processed: i32,
    pub could_operate: bool,
    pub received_energy: u64,
    pub last_received: u64,
    pub last_processed: f64,
}

/// The tanks a tick works on: polonium in, antimatter out (mB).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tanks {
    pub input: i64,
    pub output: i64,
}

impl Core {
    /// `SPSCache.merge`: progress and processed add, could operate ors, the energy adds (clamped), the max of the last.
    pub fn merge(&mut self, other: &Core) {
        self.progress += other.progress;
        self.input_processed = self.input_processed.wrapping_add(other.input_processed);
        self.could_operate |= other.could_operate;
        self.received_energy = add_clamped(self.received_energy, other.received_energy);
        self.last_processed = self.last_processed.max(other.last_processed);
    }

    /// `canOperate`: polonium in and room for antimatter.
    pub fn can_operate(tanks: Tanks, config: &SpsConfig) -> bool {
        tanks.input > 0 && config.output_tank_capacity - tanks.output > 0
    }

    /// `canSupplyCoilEnergy` for a port with a coil: it could operate last tick or can now.
    pub fn can_supply(&self, tanks: Tanks, config: &SpsConfig) -> bool {
        self.could_operate || Self::can_operate(tanks, config)
    }

    /// `supplyCoilEnergy`: a port's Joules for the next tick.
    pub fn supply(&mut self, joules: u64) {
        self.received_energy = add_clamped(self.received_energy, joules);
    }

    /// `SPSMultiblockData.tick`'s process: with polonium, room and Joules received, `energy / energyPerInput` mB more
    /// of progress, its whole part processed now (all that is needed to fill the output tank once that is reached);
    /// then the Joules received become the last and the next tick's start from none. Whether it changed what it saves
    /// (`markDirty`).
    pub fn tick(&mut self, tanks: &mut Tanks, config: &SpsConfig) -> bool {
        let before = *self;
        let mut processed = 0.0;
        self.could_operate = Self::can_operate(*tanks, config);
        if self.could_operate && self.received_energy > 0 {
            let per = i64::from(config.input_per_antimatter);
            let needed_out = config.output_tank_capacity - tanks.output;
            let input_needed = (per - i64::from(self.input_processed)).wrapping_add(per.wrapping_mul(needed_out - 1));
            let processable = self.received_energy.min(i64::MAX as u64) as f64 / config.energy_per_input as f64;
            if processable + self.progress >= input_needed as f64 {
                processed = self.process(input_needed, tanks, config) as f64;
                self.progress = 0.0;
            } else {
                processed = processable;
                self.progress += processable;
                let to_process = clamp_to_long(self.progress);
                let actual = self.process(to_process, tanks, config);
                if actual < to_process {
                    let dif = to_process - actual;
                    self.progress -= dif as f64;
                    processed -= dif as f64;
                }
                self.progress %= 1.0;
            }
        }
        self.last_received = self.received_energy;
        self.received_energy = 0;
        self.last_processed = processed;
        self.progress != before.progress || self.input_processed != before.input_processed
    }

    /// `process`: up to `operations` mB of polonium out of the input tank, each `inputPerAntimatter` of them an mB of
    /// antimatter into the output tank (no more than it holds). The polonium taken.
    fn process(&mut self, operations: i64, tanks: &mut Tanks, config: &SpsConfig) -> i64 {
        if operations <= 0 {
            return 0;
        }
        let processed = operations.min(tanks.input.max(0));
        tanks.input -= processed;
        self.input_processed = self.input_processed.saturating_add(processed.min(i64::from(i32::MAX)) as i32);
        let per = config.input_per_antimatter;
        if self.input_processed >= per {
            let add = i64::from(self.input_processed / per);
            tanks.output += add.min((config.output_tank_capacity - tanks.output).max(0));
            self.input_processed %= per;
        }
        processed
    }

    /// `getProcessRate`: antimatter a tick, to three places.
    pub fn process_rate(&self, config: &SpsConfig) -> f64 {
        java_round(self.last_processed / f64::from(config.input_per_antimatter) * 1000.0) as f64 / 1000.0
    }

    /// `getScaledProgress`: the way to the next mB of antimatter.
    pub fn scaled_progress(&self, config: &SpsConfig) -> f64 {
        (f64::from(self.input_processed) + self.progress) / f64::from(config.input_per_antimatter)
    }

    /// `lastProcessed > 0`: the screen's and the casings' active.
    pub fn active(&self) -> bool {
        self.last_processed > 0.0
    }
}

/// `getCoilLevel`: a coil's laser level for the Joules it passed.
pub fn coil_level(joules: u64) -> i32 {
    if joules == 0 {
        return 0;
    }
    1 + ((((joules as f64).log10() - 3.0) * 1.8) as i32).max(0)
}

/// `MathUtils.addClamped` on Joules.
fn add_clamped(a: u64, b: u64) -> u64 {
    a.saturating_add(b).min(i64::MAX as u64)
}

/// `MathUtils.clampToLong`.
fn clamp_to_long(d: f64) -> i64 {
    d as i64
}

/// `Math.round(double)`: the floor of the value plus a half.
fn java_round(d: f64) -> i64 {
    (d + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atm10() -> SpsConfig {
        SpsConfig { energy_per_input: 100_000, ..SpsConfig::default() }
    }

    fn grid_nodes(min: Pos, edits: &[(Pos, Option<Node>)]) -> BTreeMap<Pos, Node> {
        let cuboid = Cuboid::new(min, [min[0] + 6, min[1] + 6, min[2] + 6]);
        let mut nodes = BTreeMap::new();
        for p in cuboid.positions() {
            if matches!(requirement(&cuboid, p), Requirement::Frame | Requirement::Other) {
                nodes.insert(p, Node::Frame);
            }
        }
        for (p, n) in edits {
            match n {
                Some(n) => nodes.insert(*p, *n),
                None => nodes.remove(p),
            };
        }
        nodes
    }

    fn inside(coils: &'static [Pos]) -> impl FnMut(Pos) -> Inner {
        move |p| if coils.contains(&p) { Inner::Internal } else { Inner::Air }
    }

    #[test]
    fn forms_as_the_oracle_does() {
        let o = [0, 0, 0];
        // A port in the west face with a coil behind it, another in the north face.
        let base = [([0, 3, 3], Some(Node::Valve)), ([3, 3, 0], Some(Node::Valve))];
        let f = form(&grid_nodes(o, &base), inside(&[[1, 3, 3]])).unwrap();
        assert_eq!(f.cuboid, Cuboid::new([0, 0, 0], [6, 6, 6]));
        assert_eq!(coils(&f.valves, &f.internal), vec![([0, 3, 3], [1, 3, 3])]);
        // The corners filled, glass in a side, a port in an ignored corner: it forms.
        let mut full = grid_nodes(o, &base);
        for p in Cuboid::new(o, [6, 6, 6]).positions() {
            if requirement(&Cuboid::new(o, [6, 6, 6]), p) == Requirement::Ignored {
                full.insert(p, Node::Frame);
            }
        }
        assert!(form(&full, inside(&[])).is_ok());
        assert!(form(&grid_nodes(o, &[([6, 3, 3], Some(Node::Structural))]), inside(&[])).is_ok());
        assert!(form(&grid_nodes(o, &[([6, 6, 6], Some(Node::Valve))]), inside(&[])).is_ok());
        // A coil with no port, glass or a port in the ring, a hole in it or in a side, dirt inside: it doesn't.
        assert_eq!(form(&grid_nodes(o, &[]), inside(&[[1, 3, 3]])), Err(Failure::DisconnectedCoil));
        assert_eq!(form(&grid_nodes(o, &base), inside(&[[1, 3, 2]])), Err(Failure::DisconnectedCoil));
        assert_eq!(form(&grid_nodes(o, &[([6, 1, 1], Some(Node::Structural))]), inside(&[])), Err(Failure::InvalidFrame([6, 1, 1])));
        assert_eq!(form(&grid_nodes(o, &[([6, 1, 1], Some(Node::Valve))]), inside(&[])), Err(Failure::InvalidFrame([6, 1, 1])));
        assert_eq!(form(&grid_nodes(o, &[([6, 6, 3], Some(Node::Valve))]), inside(&[])), Err(Failure::InvalidFrame([6, 6, 3])));
        assert!(form(&grid_nodes(o, &[([6, 1, 1], None)]), inside(&[])).is_err());
        assert!(form(&grid_nodes(o, &[([6, 3, 3], None)]), inside(&[])).is_err());
        assert_eq!(form(&grid_nodes(o, &[]), |p| if p == [3, 3, 3] { Inner::Invalid } else { Inner::Air }), Err(Failure::InvalidInner([3, 3, 3])));
    }

    #[test]
    fn an_ultimate_cube_makes_progress_as_the_oracle_does() {
        // 256 000 J a tick: 2.56 mB of polonium, the input kept full.
        let config = atm10();
        let mut core = Core::default();
        let mut processed = Vec::new();
        let mut progress = Vec::new();
        let mut out = 0;
        let mut first = None;
        for tick in 3..=400 {
            core.supply(256_000);
            let mut tanks = Tanks { input: 2000, output: out };
            core.tick(&mut tanks, &config);
            out = tanks.output;
            processed.push(core.input_processed);
            progress.push(core.progress);
            if tick == 392 {
                assert_eq!((core.input_processed, core.progress), (998, 0.4000000000000208));
            }
            if out == 1 && first.is_none() {
                first = Some(tick);
            }
        }
        assert_eq!(first, Some(393));
        assert_eq!(&processed[..10], &[2, 5, 7, 10, 12, 15, 17, 20, 23, 25]);
        assert_eq!(&progress[..3], &[0.56, 0.1200000000000001, 0.6800000000000002]);
        assert_eq!((core.input_processed, core.progress, out), (18, 0.8800000000000212, 1));
        assert_eq!(core.last_processed, 2.56);
        assert_eq!(core.process_rate(&config), 0.003);
    }

    #[test]
    fn a_creative_cube_takes_the_whole_input_and_stops_when_full() {
        let config = atm10();
        let mut core = Core::default();
        let mut tanks = Tanks { input: 2000, output: 0 };
        for _ in 0..500 {
            core.supply(1_000_000_000);
            core.tick(&mut tanks, &config);
            tanks.input = 2000;
        }
        assert_eq!((tanks.output, core.progress, core.input_processed, core.last_processed), (1000, 0.0, 0, 2000.0));
        assert_eq!(core.process_rate(&config), 2.0);
        core.supply(1_000_000_000);
        core.tick(&mut tanks, &config);
        assert!(!core.could_operate && !core.can_supply(tanks, &config));
        assert_eq!(core.last_processed, 0.0);
    }
}
