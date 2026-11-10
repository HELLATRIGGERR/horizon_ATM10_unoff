//! Mekanism's Electric Pump (`TileEntityElectricPump`) and Fluidic Plenisher
//! (`TileEntityFluidicPlenisher`): machines of the world's fluids.
//!
//! The pump takes a source block (any block a bucket picks a source fluid
//! from) every `19` ticks of its Joules into a 10 000 mB tank: the block
//! under it first, then, in a shuffled order, the sources it took before
//! (`recurring_nodes`, which an infinite water source stays among) and the
//! sources beside them within `[pump] range` blocks, one fluid at a time
//! (`activeType`); it gives its tank up into the fluid handler above it at
//! `256 * (1 + speed upgrades)` mB a tick. Water stays in the world while the
//! `waterSourceConversion` rule makes it endless (`pumpInfiniteFluidSources`
//! off, ATM10's), lava while `lavaSourceConversion` does; with a filter
//! upgrade water becomes `heavyWaterAmount` mB of heavy water (50 on ATM10).
//! Its Joules go only from its first source on (`activeType`): an idle pump
//! counts its ticks for nothing.
//!
//! The plenisher takes 1000 mB of its 10 000 mB tank (filled from above or
//! its slot) out into a block every 20 ticks: it walks the space under it
//! (`activeNodes` in the order it met them, never up), one block an
//! operation, up to `maxPlenisherNodes` blocks; then it only keeps the block
//! under it full. While it walks, a tick takes its Joules; after, only a
//! placement does.
//!
//! The world is the caller's (`PumpWorld`, `PlenisherWorld`); fluids are
//! their ids, amounts mB in Java's `int`.
use crate::container::{Automation, EnergyContainer};
use crate::machine::Upgrade;
use crate::sides::RelativeSide;
use std::collections::BTreeSet;

/// `TileEntityElectricPump.BASE_TICKS_REQUIRED`.
pub const PUMP_TICKS: u32 = 19;
/// `TileEntityFluidicPlenisher.BASE_TICKS_REQUIRED`.
pub const PLENISHER_TICKS: u32 = 20;
/// Both tanks' `MAX_FLUID`.
pub const TANK: i32 = 10_000;
/// `BASE_OUTPUT_RATE`: mB a tick the pump gives up, times one more than its speed upgrades.
pub const BASE_OUTPUT: i32 = 256;
/// A bucket, what one block holds.
pub const BUCKET: i32 = 1000;
pub const WATER: &str = "minecraft:water";
pub const LAVA: &str = "minecraft:lava";
pub const HEAVY_WATER: &str = "mekanism:heavy_water";

/// `EnumUtils.DIRECTIONS` (`Direction.values()`): down, up, north, south, west, east.
pub const DIRECTIONS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
/// The plenisher's `dirs`: every direction but up.
pub const PLENISHER_DIRECTIONS: [[i32; 3]; 5] = [[0, -1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// `general.toml` `[pump]`, the two machines' `machine-usage.toml` and
/// `machine-storage.toml` keys, and the game rules the pump reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PumpConfig {
    /// `range`: the farthest a pumped block may be, 1..=512.
    pub range: u32,
    /// `pumpInfiniteFluidSources`.
    pub infinite_sources: bool,
    /// `heavyWaterAmount`: mB of heavy water a block of water gives with a filter upgrade, 1..=1000.
    pub heavy_water_amount: i32,
    /// `maxPlenisherNodes`: blocks the plenisher walks before it stops, 1..=1 000 000.
    pub max_plenisher_nodes: u32,
    /// `electricPump`, `fluidicPlenisher`: base Joules a tick and store.
    pub pump_usage: u64,
    pub pump_storage: u64,
    pub plenisher_usage: u64,
    pub plenisher_storage: u64,
    /// `GameRules.RULE_WATER_SOURCE_CONVERSION` and `RULE_LAVA_SOURCE_CONVERSION`
    /// (vanilla's defaults: water on, lava off).
    pub water_source_conversion: bool,
    pub lava_source_conversion: bool,
}

impl Default for PumpConfig {
    fn default() -> Self {
        Self {
            range: 80,
            infinite_sources: false,
            heavy_water_amount: 10,
            max_plenisher_nodes: 4000,
            pump_usage: 100,
            pump_storage: 40_000,
            plenisher_usage: 100,
            plenisher_storage: 40_000,
            water_source_conversion: true,
            lava_source_conversion: false,
        }
    }
}

impl PumpConfig {
    pub fn from_tables(general: Option<&toml::Table>, usage: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let int = |t: Option<&toml::Table>, key: &str| t.and_then(|t| t.get(key)).and_then(toml::Value::as_integer);
        let pump = general.and_then(|g| g.get("pump")).and_then(toml::Value::as_table);
        if let Some(v) = int(pump, "range").filter(|v| (1..=512).contains(v)) {
            config.range = v as u32;
        }
        if let Some(v) = pump.and_then(|p| p.get("pumpInfiniteFluidSources")).and_then(toml::Value::as_bool) {
            config.infinite_sources = v;
        }
        if let Some(v) = int(pump, "heavyWaterAmount").filter(|v| (1..=1000).contains(v)) {
            config.heavy_water_amount = v as i32;
        }
        if let Some(v) = int(pump, "maxPlenisherNodes").filter(|v| (1..=1_000_000).contains(v)) {
            config.max_plenisher_nodes = v as u32;
        }
        if let Some(v) = int(usage, "electricPump").filter(|v| *v >= 0) {
            config.pump_usage = v as u64;
        }
        if let Some(v) = int(usage, "fluidicPlenisher").filter(|v| *v >= 0) {
            config.plenisher_usage = v as u64;
        }
        if let Some(v) = int(storage, "electricPump").filter(|v| *v >= 1) {
            config.pump_storage = v as u64;
        }
        if let Some(v) = int(storage, "fluidicPlenisher").filter(|v| *v >= 1) {
            config.plenisher_storage = v as u64;
        }
        config
    }
}

/// The two machines (`MekanismBlocks.ELECTRIC_PUMP`, `FLUIDIC_PLENISHER`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FluidMachine {
    Pump,
    Plenisher,
}

/// Their slots: the fluid slot (the pump's fills a container from the tank,
/// the plenisher's empties one into it), what it became, energy, then the
/// upgrade component's input and output.
pub const INPUT_SLOT: usize = 0;
pub const OUTPUT_SLOT: usize = 1;
pub const ENERGY_SLOT: usize = 2;
pub const UPGRADE_SLOT: usize = 3;
pub const UPGRADE_OUT_SLOT: usize = 4;
pub const SLOTS: usize = 5;
/// Their Joules come in through the back (`MachineEnergyContainer.input`, `RelativeSide.BACK`).
pub const ENERGY_SIDE: RelativeSide = RelativeSide::Back;
/// Their tank is on the top (`FluidTankHelper.forSide(facing).addTank(fluidTank, TOP)`).
pub const TANK_SIDE: RelativeSide = RelativeSide::Top;

impl FluidMachine {
    pub const ALL: [Self; 2] = [Self::Pump, Self::Plenisher];

    pub fn id(self) -> &'static str {
        match self {
            Self::Pump => "mekanism:electric_pump",
            Self::Plenisher => "mekanism:fluidic_plenisher",
        }
    }

    pub fn of_block(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    /// `BASE_TICKS_REQUIRED`.
    pub fn base_ticks(self) -> u32 {
        match self {
            Self::Pump => PUMP_TICKS,
            Self::Plenisher => PLENISHER_TICKS,
        }
    }

    /// Its base Joules a tick and store.
    pub fn energy(self, config: &PumpConfig) -> (u64, u64) {
        match self {
            Self::Pump => (config.pump_usage, config.pump_storage),
            Self::Plenisher => (config.plenisher_usage, config.plenisher_storage),
        }
    }

    /// The upgrades it takes: the pump's speed, energy and filter
    /// (`withSupportedUpgrades`), the plenisher's speed and energy (`SPEED_ENERGY`).
    pub fn upgrades(self) -> &'static [Upgrade] {
        match self {
            Self::Pump => &[Upgrade::Speed, Upgrade::Energy, Upgrade::Filter],
            Self::Plenisher => &[Upgrade::Speed, Upgrade::Energy],
        }
    }
}

/// A machine's tank: its fluid and mB.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tank {
    pub fluid: Option<String>,
    pub amount: i32,
}

impl Tank {
    pub fn is_empty(&self) -> bool {
        self.fluid.is_none() || self.amount <= 0
    }

    pub fn needed(&self) -> i32 {
        TANK - self.amount.max(0)
    }

    pub fn holds(&self, fluid: &str) -> bool {
        !self.is_empty() && self.fluid.as_deref() == Some(fluid)
    }

    /// `insert` (any automation the tank lets in): what of `amount` mB of
    /// `fluid` fits, into an empty tank or one of the same fluid.
    pub fn insert(&mut self, fluid: &str, amount: i32) -> i32 {
        if amount <= 0 || !(self.is_empty() || self.holds(fluid)) {
            return 0;
        }
        let took = amount.min(self.needed());
        if took > 0 {
            if self.is_empty() {
                self.fluid = Some(fluid.to_owned());
                self.amount = 0;
            }
            self.amount += took;
        }
        took
    }

    /// `extract` of up to `amount` mB: what it gave.
    pub fn extract(&mut self, amount: i32) -> i32 {
        if self.is_empty() || amount <= 0 {
            return 0;
        }
        let took = amount.min(self.amount);
        self.amount -= took;
        if self.amount == 0 {
            self.fluid = None;
        }
        took
    }
}

/// `MachineEnergyContainer`'s draw of a tick's Joules: whether all of them
/// are there (`extract(SIMULATE) == energyPerTick`).
fn has_tick(energy: &mut EnergyContainer, per_tick: u64) -> bool {
    energy.extract(per_tick, true, Automation::Internal) == per_tick
}

fn take_tick(energy: &mut EnergyContainer, per_tick: u64) -> u64 {
    energy.extract(per_tick, false, Automation::Internal)
}

fn offset(pos: [i32; 3], by: [i32; 3]) -> [i32; 3] {
    [pos[0] + by[0], pos[1] + by[1], pos[2] + by[2]]
}

// ---- The Electric Pump --------------------------------------------------------------

/// What the pump reads and changes of the world.
pub trait PumpWorld {
    /// The source fluid a bucket picks up at `pos` (`BucketPickup` over a
    /// source fluid state: a liquid's source block, a waterlogged block),
    /// none where there is none or the block isn't loaded.
    fn source(&mut self, pos: [i32; 3]) -> Option<String>;
    /// `pickupBlock`: takes it out of the world; the fluid its bucket holds,
    /// none when nothing came.
    fn pick_up(&mut self, pos: [i32; 3]) -> Option<String>;
    /// `Collections.shuffle` of the nodes to look at.
    fn shuffle(&mut self, nodes: &mut [[i32; 3]]);
}

/// What the pump saves: `progress`, `fluid` (its `activeType`) and
/// `recurring_nodes`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pump {
    pub progress: u32,
    pub active: Option<String>,
    pub recurring: BTreeSet<[i32; 3]>,
}

/// One tick's settings of a pump.
#[derive(Clone, Copy, Debug)]
pub struct PumpRun<'a> {
    pub config: &'a PumpConfig,
    /// `getEnergyPerTick` with its upgrades.
    pub per_tick: u64,
    /// `ticksRequired`: `getTicks(19)` with its speed upgrades.
    pub ticks: u32,
    /// A filter upgrade is installed.
    pub filter: bool,
    /// `canFunction`: redstone control (IGNORED here, so always).
    pub can_function: bool,
}

impl Pump {
    /// `estimateIncrementAmount`: what a block will give, by what the tank holds.
    pub fn increment(tank: &Tank, config: &PumpConfig) -> i32 {
        if tank.holds(HEAVY_WATER) { config.heavy_water_amount } else { BUCKET }
    }

    /// `onUpdateServer` from the operation on (the slots before it, the
    /// emission after it are the caller's): with room for a block's fluid
    /// and a tick's Joules it counts a tick, taking the Joules once it has a
    /// fluid; at `ticks` it takes a block (`suck`), or starts over
    /// (`reset`). Joules used this tick.
    pub fn tick(&mut self, origin: [i32; 3], tank: &mut Tank, energy: &mut EnergyContainer, run: &PumpRun<'_>, world: &mut impl PumpWorld) -> u64 {
        let mut used = 0;
        if run.can_function && (tank.is_empty() || Self::increment(tank, run.config) <= tank.needed()) && has_tick(energy, run.per_tick) {
            if self.active.is_some() {
                used = take_tick(energy, run.per_tick);
            }
            self.progress += 1;
            if self.progress >= run.ticks {
                self.progress = 0;
                if self.suck(origin, tank, run, world) {
                    if used == 0 {
                        used = take_tick(energy, run.per_tick);
                    }
                } else {
                    self.reset();
                }
            }
        }
        used
    }

    /// `reset`: no fluid, no nodes (a sneaking Configurator or the screen's button).
    pub fn reset(&mut self) {
        self.active = None;
        self.recurring.clear();
    }

    /// `suck()`: the block under it, then each node in a shuffled order and
    /// the blocks beside it within range; a node that gives nothing more is
    /// forgotten.
    fn suck(&mut self, origin: [i32; 3], tank: &mut Tank, run: &PumpRun<'_>, world: &mut impl PumpWorld) -> bool {
        if self.suck_at(offset(origin, [0, -1, 0]), true, tank, run, world) {
            return true;
        }
        let mut nodes: Vec<[i32; 3]> = self.recurring.iter().copied().collect();
        world.shuffle(&mut nodes);
        let range = f64::from(run.config.range);
        for node in nodes {
            if self.suck_at(node, false, tank, run, world) {
                return true;
            }
            for d in DIRECTIONS {
                let at = offset(node, d);
                let [dx, dy, dz] = [at[0] - origin[0], at[1] - origin[1], at[2] - origin[2]].map(f64::from);
                if (dx * dx + dy * dy + dz * dz).sqrt() <= range && self.suck_at(at, true, tank, run, world) {
                    return true;
                }
            }
            self.recurring.remove(&node);
        }
        false
    }

    /// `getOutput`: a bucket of the fluid, or with a filter water's heavy water.
    fn output(fluid: &str, filter: bool, config: &PumpConfig) -> (String, i32) {
        if filter && fluid == WATER { (HEAVY_WATER.to_owned(), config.heavy_water_amount) } else { (fluid.to_owned(), BUCKET) }
    }

    /// `validFluid`: of the fluid it pumps (or any while it pumps none), and
    /// all of it fits the tank.
    fn valid(&self, fluid: &str, amount: i32, tank: &Tank) -> bool {
        if amount <= 0 || self.active.as_deref().is_some_and(|a| a != fluid) {
            return false;
        }
        tank.is_empty() || (tank.holds(fluid) && amount <= tank.needed())
    }

    /// `shouldPump`: whether the block leaves the world.
    fn should_pump(fluid: &str, config: &PumpConfig) -> bool {
        if config.infinite_sources {
            return true;
        }
        match fluid {
            WATER => !config.water_source_conversion,
            LAVA => !config.lava_source_conversion,
            _ => true,
        }
    }

    /// `suck(pos, hasFilter, addRecurring)`.
    fn suck_at(&mut self, pos: [i32; 3], add: bool, tank: &mut Tank, run: &PumpRun<'_>, world: &mut impl PumpWorld) -> bool {
        let Some(source) = world.source(pos) else {
            return false;
        };
        let (mut fluid, mut amount) = Self::output(&source, run.filter, run.config);
        if !self.valid(&fluid, amount, tank) {
            return false;
        }
        if Self::should_pump(&source, run.config) {
            let Some(picked) = world.pick_up(pos) else {
                return false;
            };
            if picked != source {
                (fluid, amount) = Self::output(&picked, run.filter, run.config);
                if !self.valid(&fluid, amount, tank) {
                    return false;
                }
            }
        }
        self.active = Some(fluid.clone());
        if add {
            self.recurring.insert(pos);
        }
        tank.insert(&fluid, amount);
        true
    }
}

/// `256 * (1 + speed)`: what the pump gives up a tick.
pub fn output_rate(speed: u32) -> i32 {
    BASE_OUTPUT.saturating_mul(1 + speed as i32)
}

// ---- The Fluidic Plenisher ----------------------------------------------------------

/// What stands at a position, as the plenisher's `canReplace` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Air,
    /// A source fluid state (a liquid's source, a waterlogged block).
    Source,
    /// A fluid that flows.
    Flowing,
    /// A block a fluid replaces (`canBeReplaced(fluid)`).
    Replaceable,
    /// A block that takes the fluid in (`LiquidBlockContainer.canPlaceLiquid`).
    Container,
    Solid,
}

/// What the plenisher reads and changes of the world.
pub trait PlenisherWorld {
    /// What is at `pos` for the tank's fluid (none while it holds none); none
    /// where the block isn't loaded.
    fn spot(&mut self, pos: [i32; 3], fluid: Option<&str>) -> Option<Spot>;
    /// `WorldUtils.tryPlaceContainedLiquid`: a source of `fluid` at `pos`
    /// (or its vapour in an ultra-warm dimension); whether it went.
    fn place(&mut self, pos: [i32; 3], fluid: &str) -> bool;
}

/// What the plenisher saves: `progress`, `finished`, `active_nodes` (in
/// the order it met them) and `used_nodes`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plenisher {
    pub progress: u32,
    pub finished: bool,
    pub active: Vec<[i32; 3]>,
    pub used: BTreeSet<[i32; 3]>,
}

impl Plenisher {
    /// `onUpdateServer` from the operation on: with fluid and a tick's
    /// Joules it counts a tick (taking the Joules while it walks); at
    /// `ticks` it fills the block under it once it walked, else walks on.
    /// Joules used this tick.
    pub fn tick(&mut self, origin: [i32; 3], tank: &mut Tank, energy: &mut EnergyContainer, per_tick: u64, ticks: u32, max_nodes: u32, world: &mut impl PlenisherWorld) -> u64 {
        let mut used = 0;
        if tank.is_empty() || !has_tick(energy, per_tick) {
            return used;
        }
        if !self.finished {
            used = take_tick(energy, per_tick);
        }
        self.progress += 1;
        if self.progress >= ticks {
            self.progress = 0;
            if self.finished {
                let below = offset(origin, [0, -1, 0]);
                if self.can_replace(below, false, false, tank, world) && tank.amount >= BUCKET && self.place(below, tank, world) {
                    used = take_tick(energy, per_tick);
                }
            } else {
                self.plenish(origin, tank, max_nodes, world);
            }
        }
        used
    }

    /// `reset`.
    pub fn reset(&mut self) {
        self.active.clear();
        self.used.clear();
        self.finished = false;
    }

    fn place(&self, pos: [i32; 3], tank: &mut Tank, world: &mut impl PlenisherWorld) -> bool {
        let Some(fluid) = tank.fluid.clone() else {
            return false;
        };
        if !world.place(pos, &fluid) {
            return false;
        }
        tank.extract(BUCKET);
        true
    }

    /// `doPlenish`: one node of the walk.
    fn plenish(&mut self, origin: [i32; 3], tank: &mut Tank, max_nodes: u32, world: &mut impl PlenisherWorld) {
        if self.used.len() >= max_nodes as usize {
            self.finished = true;
            return;
        }
        if self.active.is_empty() {
            if !self.used.is_empty() {
                self.finished = true;
                return;
            }
            let below = offset(origin, [0, -1, 0]);
            if !self.can_replace(below, true, true, tank, world) {
                self.finished = true;
                return;
            }
            self.active.push(below);
        }
        let mut done = Vec::new();
        let nodes = self.active.clone();
        for node in nodes {
            done.push(node);
            if world.spot(node, tank.fluid.as_deref()).is_none() {
                continue;
            }
            if self.can_replace(node, true, false, tank, world) && tank.amount >= BUCKET {
                self.place(node, tank, world);
            }
            for d in PLENISHER_DIRECTIONS {
                let at = offset(node, d);
                if world.spot(at, tank.fluid.as_deref()).is_some() && self.can_replace(at, true, true, tank, world) && !self.active.contains(&at) {
                    self.active.push(at);
                }
            }
            break;
        }
        for node in done {
            self.used.insert(node);
            self.active.retain(|a| *a != node);
        }
    }

    /// `canReplace`: not a node it used; air, a flowing fluid or what the
    /// fluid replaces or fills; a source only while it walks.
    fn can_replace(&self, pos: [i32; 3], check_nodes: bool, pathfinding: bool, tank: &Tank, world: &mut impl PlenisherWorld) -> bool {
        if check_nodes && self.used.contains(&pos) {
            return false;
        }
        match world.spot(pos, tank.fluid.as_deref()) {
            Some(Spot::Air | Spot::Flowing | Spot::Replaceable | Spot::Container) => true,
            Some(Spot::Source) => pathfinding,
            Some(Spot::Solid) | None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A world of fluid blocks: each position's fluid and whether it is a source.
    #[derive(Default)]
    struct World {
        fluids: HashMap<[i32; 3], (String, bool)>,
        solid: BTreeSet<[i32; 3]>,
        picked: Vec<[i32; 3]>,
        placed: Vec<[i32; 3]>,
    }

    impl PumpWorld for World {
        fn source(&mut self, pos: [i32; 3]) -> Option<String> {
            self.fluids.get(&pos).filter(|f| f.1).map(|f| f.0.clone())
        }
        fn pick_up(&mut self, pos: [i32; 3]) -> Option<String> {
            self.picked.push(pos);
            self.fluids.remove(&pos).map(|f| f.0)
        }
        fn shuffle(&mut self, _nodes: &mut [[i32; 3]]) {}
    }

    impl PlenisherWorld for World {
        fn spot(&mut self, pos: [i32; 3], _fluid: Option<&str>) -> Option<Spot> {
            if pos[1] < -63 || self.solid.contains(&pos) {
                return Some(Spot::Solid);
            }
            Some(match self.fluids.get(&pos) {
                Some((_, true)) => Spot::Source,
                Some((_, false)) => Spot::Flowing,
                None => Spot::Air,
            })
        }
        fn place(&mut self, pos: [i32; 3], fluid: &str) -> bool {
            self.placed.push(pos);
            self.fluids.insert(pos, (fluid.to_owned(), true));
            true
        }
    }

    fn atm10() -> PumpConfig {
        PumpConfig { heavy_water_amount: 50, ..PumpConfig::default() }
    }

    fn pool(world: &mut World, fluid: &str, xs: std::ops::Range<i32>, ys: &[i32], zs: std::ops::Range<i32>) {
        for x in xs {
            for &y in ys {
                for z in zs.clone() {
                    world.fluids.insert([x, y, z], (fluid.to_owned(), true));
                }
            }
        }
    }

    #[test]
    fn the_pump_takes_a_block_every_19_ticks() {
        // Measured on the oracle (g2/w2, P1): over a 5x5x2 lava pool with 40 000 J, nothing for 18
        // ticks and no Joules; on the 19th the block under it goes (100 J) and 1000 mB of lava come
        // in; from then 100 J a tick, a block every 19 ticks.
        let config = atm10();
        let mut world = World::default();
        pool(&mut world, LAVA, 0..5, &[-62, -61], 18..23);
        let mut pump = Pump::default();
        let mut tank = Tank::default();
        let mut energy = EnergyContainer::basic(40_000);
        energy.stored = 40_000;
        let run = PumpRun { config: &config, per_tick: 100, ticks: PUMP_TICKS, filter: false, can_function: true };
        let origin = [2, -60, 20];
        let mut rows = Vec::new();
        for _ in 0..38 {
            let used = pump.tick(origin, &mut tank, &mut energy, &run, &mut world);
            rows.push((used, tank.amount));
            // The emission into the tank above takes it all.
            tank.extract(output_rate(0) * 4);
        }
        assert!(rows[..18].iter().all(|r| *r == (0, 0)));
        assert_eq!(rows[18], (100, 1000));
        assert!(rows[19..37].iter().all(|r| r.0 == 100));
        assert_eq!(rows[37], (100, 1000));
        assert_eq!(world.picked[0], [2, -61, 20], "the block under it first");
        assert_eq!(world.picked[1], [2, -62, 20], "then below that, beside the node it took");
        assert_eq!(energy.stored, 40_000 - 20 * 100);
        assert_eq!(pump.active.as_deref(), Some(LAVA));
        assert_eq!(pump.recurring.len(), 2);
    }

    #[test]
    fn water_is_endless_and_a_filter_makes_heavy_water() {
        // P2 and P3: the water under it stays a source; 1000 mB of water an operation, or 50 of
        // heavy water with a filter upgrade.
        let config = atm10();
        let mut world = World::default();
        pool(&mut world, WATER, 10..13, &[-61], 18..21);
        let origin = [11, -60, 19];
        for (filter, fluid, each) in [(false, WATER, 1000), (true, HEAVY_WATER, 50)] {
            let mut pump = Pump::default();
            let mut tank = Tank::default();
            let mut energy = EnergyContainer::basic(40_000);
            energy.stored = 40_000;
            let run = PumpRun { config: &config, per_tick: 100, ticks: PUMP_TICKS, filter, can_function: true };
            for _ in 0..38 {
                pump.tick(origin, &mut tank, &mut energy, &run, &mut world);
            }
            assert_eq!((tank.fluid.as_deref(), tank.amount), (Some(fluid), 2 * each));
            assert_eq!(energy.stored, 40_000 - 20 * 100);
        }
        assert!(world.picked.is_empty(), "no water left the world");
        assert_eq!(Pump::increment(&Tank { fluid: Some(HEAVY_WATER.into()), amount: 50 }, &config), 50);
        let endless = PumpConfig { infinite_sources: true, ..config };
        assert!(Pump::should_pump(WATER, &endless));
        assert!(!Pump::should_pump(WATER, &config) && Pump::should_pump(LAVA, &config));
    }

    #[test]
    fn a_pump_without_room_or_sources_waits_and_starts_over() {
        let config = atm10();
        let mut world = World::default();
        let run = PumpRun { config: &config, per_tick: 100, ticks: PUMP_TICKS, filter: false, can_function: true };
        let mut pump = Pump { active: Some(LAVA.into()), ..Pump::default() };
        pump.recurring.insert([0, -70, 0]);
        let mut tank = Tank::default();
        let mut energy = EnergyContainer::basic(40_000);
        energy.stored = 40_000;
        for _ in 0..19 {
            pump.tick([0, 0, 0], &mut tank, &mut energy, &run, &mut world);
        }
        assert_eq!(pump, Pump::default(), "nothing to take: reset");
        // A tank without room for a bucket: it doesn't count.
        let mut full = Tank { fluid: Some(LAVA.into()), amount: 9_500 };
        pool(&mut world, LAVA, 0..1, &[-1], 0..1);
        assert_eq!(pump.tick([0, 0, 0], &mut full, &mut energy, &run, &mut world), 0);
        assert_eq!(pump.progress, 0);
        // Another fluid than the one it pumps is no use: water under a lava pump.
        let mut lava_pump = Pump { active: Some(LAVA.into()), ..Pump::default() };
        world.fluids.insert([0, -1, 0], (WATER.into(), true));
        let mut tank = Tank::default();
        for _ in 0..19 {
            lava_pump.tick([0, 0, 0], &mut tank, &mut energy, &run, &mut world);
        }
        assert!(tank.is_empty());
        assert_eq!(output_rate(0), 256);
        assert_eq!(output_rate(8), 2304);
    }

    #[test]
    fn the_plenisher_walks_down_and_out() {
        // Measured on the oracle (g2/w2, L1): 10 000 mB of lava over a 3x3x2 hole in an open layer:
        // 100 J every tick while it walks, a block every 20 ticks in the order it met them (down,
        // north, south, west, east), and nothing more with its tank empty.
        let mut world = World::default();
        for x in 0..40 {
            for z in 10..40 {
                world.solid.insert([x, -63, z]);
                if !(22..=24).contains(&x) || !(24..=26).contains(&z) {
                    world.solid.insert([x, -62, z]);
                }
            }
        }
        let mut plenisher = Plenisher::default();
        let mut tank = Tank { fluid: Some(LAVA.into()), amount: 10_000 };
        let mut energy = EnergyContainer::basic(40_000);
        energy.stored = 40_000;
        let mut used = Vec::new();
        for _ in 0..220 {
            used.push(plenisher.tick([23, -60, 25], &mut tank, &mut energy, 100, PLENISHER_TICKS, 4000, &mut world));
        }
        assert_eq!(
            world.placed,
            [
                [23, -61, 25],
                [23, -62, 25],
                [23, -61, 24],
                [23, -61, 26],
                [22, -61, 25],
                [24, -61, 25],
                [23, -62, 24],
                [23, -62, 26],
                [22, -62, 25],
                [24, -62, 25]
            ]
        );
        assert!(used[..200].iter().all(|j| *j == 100));
        assert!(used[200..].iter().all(|j| *j == 0));
        assert_eq!((energy.stored, tank.is_empty()), (20_000, true));
        assert_eq!(plenisher.used.len(), 10);
        assert!(!plenisher.finished);
    }

    #[test]
    fn the_plenisher_finishes_and_keeps_the_block_under_it_full() {
        let mut world = World::default();
        // A one-block hole under it, stone all round.
        for x in -2..=2 {
            for z in -2..=2 {
                for y in -4..=-1 {
                    if !(x == 0 && z == 0 && y == -1) {
                        world.solid.insert([x, y, z]);
                    }
                }
            }
        }
        let mut plenisher = Plenisher::default();
        let mut tank = Tank { fluid: Some(WATER.into()), amount: 10_000 };
        let mut energy = EnergyContainer::basic(40_000);
        energy.stored = 40_000;
        for _ in 0..60 {
            plenisher.tick([0, 0, 0], &mut tank, &mut energy, 100, PLENISHER_TICKS, 4000, &mut world);
        }
        assert!(plenisher.finished, "walked its one node, then found no more");
        assert_eq!(world.placed, [[0, -1, 0]]);
        assert_eq!(energy.stored, 40_000 - 40 * 100, "only while it walked");
        // The block under it empties: the next operation fills it, for a tick's Joules.
        world.fluids.remove(&[0, -1, 0]);
        let before = energy.stored;
        let mut used = 0;
        for _ in 0..20 {
            used += plenisher.tick([0, 0, 0], &mut tank, &mut energy, 100, PLENISHER_TICKS, 4000, &mut world);
        }
        assert_eq!((used, before - energy.stored, tank.amount), (100, 100, 8000));
        // `maxPlenisherNodes`: a walk of one node stops at once.
        let mut short = Plenisher { used: [[9, 9, 9]].into_iter().collect(), active: vec![[0, -1, 0]], ..Plenisher::default() };
        short.plenish([0, 0, 0], &mut Tank { fluid: Some(WATER.into()), amount: 1000 }, 1, &mut world);
        assert!(short.finished);
    }

    #[test]
    fn atm10_pump_settings() {
        let general: toml::Table = "[pump]\nrange = 80\npumpInfiniteFluidSources = false\nheavyWaterAmount = 50\nmaxPlenisherNodes = 4000\n".parse().unwrap();
        let usage: toml::Table = "electricPump = 100\nfluidicPlenisher = 100\n".parse().unwrap();
        let storage: toml::Table = "electricPump = 40000\nfluidicPlenisher = 40000\n".parse().unwrap();
        let c = PumpConfig::from_tables(Some(&general), Some(&usage), Some(&storage));
        assert_eq!((c.range, c.infinite_sources, c.heavy_water_amount, c.max_plenisher_nodes), (80, false, 50, 4000));
        assert_eq!((c.pump_usage, c.pump_storage, c.plenisher_usage, c.plenisher_storage), (100, 40_000, 100, 40_000));
        assert_eq!(PumpConfig::from_tables(None, None, None).heavy_water_amount, 10, "Mekanism's own default");
        let odd: toml::Table = "[pump]\nrange = 0\nheavyWaterAmount = 1001\n".parse().unwrap();
        let c = PumpConfig::from_tables(Some(&odd), None, None);
        assert_eq!((c.range, c.heavy_water_amount), (80, 10));
    }
}
