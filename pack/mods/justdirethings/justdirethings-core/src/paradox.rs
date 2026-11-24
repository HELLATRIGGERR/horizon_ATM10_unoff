//! The Paradox Machine (`ParadoxMachineBE`, with `BaseMachineBE`, `AreaAffectingBE` and `AreaAffectingData`): it
//! snapshots the blocks of the `justdirethings:paradox_allow` tag and the living entities in its area, and on a
//! redstone pulse (its default mode) restores what is missing since, for time fluid and Forge Energy, over ten ticks
//! for each block and entity. Every block and entity it restores adds paradox energy. The world side (the snapshot,
//! which blocks can be placed, the entities) is the caller's, through `Machine`.
use crate::{
    config::ParadoxSettings,
    redstone::{Mode, Redstone},
};

pub const BLOCK: &str = "justdirethings:paradoxmachine";
/// The tank takes only time fluid (`fluidstack.getFluid() instanceof TimeFluid`).
pub const TIME_FLUIDS: [&str; 2] = ["justdirethings:time_fluid_source", "justdirethings:time_fluid_flowing"];
/// The blocks a snapshot keeps.
pub const ALLOW_TAG: &str = "justdirethings:paradox_allow";
/// The entities it never keeps, with `c:teleporting_not_supported` and players.
pub const DENY_TAGS: [&str; 2] = ["justdirethings:paradox_deny", "c:teleporting_not_supported"];
/// `AreaAffectingBE.maxRadius` and `maxOffset`.
pub const MAX_RADIUS: f64 = 5.0;
pub const MAX_OFFSET: i32 = 9;
/// What a restored mob's data loses (`sanitizeEntityDataDeny`), with the keys this game writes the same items under.
pub const DENIED_FIELDS: [&str; 7] = ["ArmorItems", "HandItems", "Items", "SaddleItem", "Inventory", "equipment", "body_armor_item"];
/// What `paradox_restricted_mobs` keeps (`sanitizeEntityData`).
pub const RESTRICTED_FIELDS: [&str; 21] = [
    "IsBaby", "UUID", "Health", "Motion", "Rotation", "Fire", "CustomName", "NoAI", "PersistenceRequired", "Silent", "Color", "Sheared",
    "Variant", "FromBucket", "Age", "VillagerData", "Xp", "LastRestock", "RestocksToday", "Offers", "EggLayTime",
];

/// `AreaAffectingData`: the radius on each axis (steps of 0.5) and the offset of the area's centre from the machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub radius: [f64; 3],
    pub offset: [i32; 3],
    pub render: bool,
}

impl Area {
    /// `new AreaAffectingData(facing)`: the one block in front.
    pub fn facing(normal: [i32; 3]) -> Self {
        Self { radius: [0.0; 3], offset: normal, render: false }
    }

    /// `getAABB`: min and max corners. A half radius moves the area half a block, as Java does.
    pub fn bounds(&self, pos: [i32; 3]) -> ([f64; 3], [f64; 3]) {
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for a in 0..3 {
            let r = self.radius[a];
            let off = f64::from(self.offset[a]) + if r != r.floor() { 0.5 } else { 0.0 };
            min[a] = f64::from(pos[a]) + off - r;
            max[a] = f64::from(pos[a]) + off + 1.0 + r;
        }
        (min, max)
    }

    /// `findBlocksToSave`'s positions: `BlockPos.betweenClosed((int) min, (int) max - 1)`, x fastest then z then y as
    /// `betweenClosed` walks.
    pub fn blocks(&self, pos: [i32; 3]) -> Vec<[i32; 3]> {
        let (min, max) = self.bounds(pos);
        let lo = min.map(|v| v as i32);
        let hi = max.map(|v| v as i32 - 1);
        let mut out = Vec::new();
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    out.push([x, y, z]);
                }
            }
        }
        out
    }

    /// The radius button on `axis` (`ValueButtonsDouble`): half a block more or less, within 0 and the most.
    pub fn step_radius(&mut self, axis: usize, up: bool) {
        let r = &mut self.radius[axis];
        if up && *r < MAX_RADIUS {
            *r += 0.5;
        } else if !up && *r > 0.0 {
            *r -= 0.5;
        }
    }

    /// The offset button on `axis` (`ValueButtons`): one block more or less, within the most.
    pub fn step_offset(&mut self, axis: usize, up: bool) {
        let o = &mut self.offset[axis];
        *o = (*o + if up { 1 } else { -1 }).clamp(-MAX_OFFSET, MAX_OFFSET);
    }
}

/// What a snapshot keeps (`targetType`): both, blocks only, entities only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Both,
    Blocks,
    Entities,
}

impl Target {
    pub fn from_index(i: i32) -> Self {
        match i {
            1 => Self::Blocks,
            2 => Self::Entities,
            _ => Self::Both,
        }
    }
    pub fn index(self) -> i32 {
        self as i32
    }
    pub fn blocks(self) -> bool {
        self != Self::Entities
    }
    pub fn entities(self) -> bool {
        self != Self::Blocks
    }
}

/// A sound the machine plays at itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A run starts, and every 100 ticks of it (`PORTAL_AMBIENT`, 0.5, 0.25).
    Running,
    /// A run restored its blocks and entities (`EVOKER_PREPARE_SUMMON`, 0.5, 0.25).
    Restored,
    /// A run failed for want of fluid or energy (`CONDUIT_DEACTIVATE`, 0.5, 0.25).
    Failed,
}

impl Sound {
    pub fn id(self) -> &'static str {
        match self {
            Self::Running => "minecraft:block.portal.ambient",
            Self::Restored => "minecraft:entity.evoker.prepare_summon",
            Self::Failed => "minecraft:block.conduit.deactivate",
        }
    }
}

/// The world side of a run.
pub trait Machine {
    /// `hasSnapshotData`: a snapshot was taken.
    fn has_snapshot(&self) -> bool;
    /// `paradoxExists`: a paradox stands where the machine works (none can, while the paradox isn't ported).
    fn paradox_exists(&mut self) -> bool {
        false
    }
    /// A run is planned (`testRestoreBlocks` and `getEntitiesFromNBT`): the snapshot's blocks whose place can take
    /// them now and the snapshot's entities no longer in the level, kept for the run. How many of each.
    fn plan(&mut self, target: Target) -> (usize, usize);
    /// The planned blocks and entities are put back (`restoreBlocks`, `restoreEntities`).
    fn restore(&mut self);
    /// The plan is dropped.
    fn clear(&mut self);
    fn sound(&mut self, sound: Sound);
}

/// The machine's state, saved under Java's keys.
#[derive(Clone, Debug, PartialEq)]
pub struct Paradox {
    /// Forge Energy in the buffer.
    pub energy: i32,
    /// Time fluid in the tank (mB).
    pub fluid: i32,
    pub redstone: Redstone,
    pub tick_speed: i32,
    /// The operation clock (`operationTicks`, not saved).
    pub operation_ticks: i32,
    pub target: Target,
    pub render_paradox: bool,
    pub area: Area,
    pub running: bool,
    pub time_running: i32,
    pub fe_per_tick: i32,
    pub fluid_per_tick: i32,
    pub paradox_energy: f32,
    /// What the running plan holds (`restoringBlocks.size()`, `restoringEntites.size()`).
    pub planned: (usize, usize),
}

impl Paradox {
    /// A machine placed facing the side with `normal`: redstone on pulse, the area the block in front.
    pub fn new(normal: [i32; 3]) -> Self {
        Self {
            energy: 0,
            fluid: 0,
            redstone: Redstone { mode: Mode::Pulse, ..Redstone::default() },
            tick_speed: 20,
            operation_ticks: -1,
            target: Target::Both,
            render_paradox: false,
            area: Area::facing(normal),
            running: false,
            time_running: 0,
            fe_per_tick: 0,
            fluid_per_tick: 0,
            paradox_energy: 0.0,
            planned: (0, 0),
        }
    }
}

/// `getRunTime`: ten ticks a block and an entity.
pub fn run_time(blocks: usize, entities: usize) -> i32 {
    ((blocks + entities) as i32).saturating_mul(10)
}

/// `getEnergyCost`.
pub fn energy_cost(cfg: &ParadoxSettings, blocks: usize, entities: usize) -> i32 {
    (blocks as i32).wrapping_mul(cfg.rf_per_block).wrapping_add((entities as i32).wrapping_mul(cfg.rf_per_entity))
}

/// `getFluidCost`.
pub fn fluid_cost(cfg: &ParadoxSettings, blocks: usize, entities: usize) -> i32 {
    (blocks as i32).wrapping_mul(cfg.fluid_per_block).wrapping_add((entities as i32).wrapping_mul(cfg.fluid_per_entity))
}

impl Paradox {
    /// `getRunTime` while running, 300 otherwise (as Java's never-used fallback).
    pub fn run_time(&self) -> i32 {
        if self.running { run_time(self.planned.0, self.planned.1) } else { 300 }
    }

    /// `extractFluid` (`drain(amount, EXECUTE)`): what the tank gives of `amount`.
    fn extract_fluid(&mut self, amount: i32) -> i32 {
        let take = amount.clamp(0, self.fluid);
        self.fluid -= take;
        take
    }

    /// `extractEnergy(amount, false)`.
    fn extract_energy(&mut self, amount: i32) -> i32 {
        let take = amount.clamp(0, self.energy);
        self.energy -= take;
        take
    }

    /// `receiveEnergy`: what the buffer takes of `max`.
    pub fn receive_energy(&mut self, cfg: &ParadoxSettings, max: i64, simulate: bool) -> i64 {
        let take = max.clamp(0, i64::from((cfg.rf_capacity - self.energy).max(0)));
        if !simulate {
            self.energy += take as i32;
        }
        take
    }

    /// `fill`: what the tank takes of `amount` of time fluid.
    pub fn fill(&mut self, cfg: &ParadoxSettings, amount: i32, simulate: bool) -> i32 {
        let take = amount.clamp(0, (cfg.fluid_capacity - self.fluid).max(0));
        if !simulate {
            self.fluid += take;
        }
        take
    }

    /// `drain`: what leaves the tank of at most `max`.
    pub fn drain(&mut self, max: i32, simulate: bool) -> i32 {
        let take = max.clamp(0, self.fluid);
        if !simulate {
            self.fluid -= take;
        }
        take
    }

    /// `canRun`: the operation clock came round, or the machine is on pulse.
    fn can_run(&self) -> bool {
        self.operation_ticks == 0 || self.redstone.mode == Mode::Pulse
    }

    /// One server tick (`tickServer`): the clock and the redstone (`BaseMachineBE.tickServer`), then `doParadox`.
    /// A full paradox energy would spawn a paradox and empty it (`spawnParadox`); the paradox isn't ported, so the
    /// energy stays full.
    pub fn tick(&mut self, cfg: &ParadoxSettings, signal: bool, machine: &mut impl Machine) {
        if self.operation_ticks <= 0 {
            self.operation_ticks = self.tick_speed;
        }
        self.operation_ticks -= 1;
        self.redstone.evaluate(signal);
        self.start(cfg, machine);
        if !self.running {
            return;
        }
        let run_time = self.run_time();
        if self.time_running % 100 == 0 && self.time_running < run_time {
            machine.sound(Sound::Running);
        }
        let (fluid, fe) = (self.fluid_per_tick, self.fe_per_tick);
        if self.extract_fluid(fluid) == fluid && self.extract_energy(fe) == fe {
            self.time_running += 1;
        } else {
            self.stop(cfg, false, machine);
            return;
        }
        if self.time_running >= run_time {
            self.stop(cfg, true, machine);
        }
    }

    /// `startParadox`: on an active signal (a pulse is used up by asking) and the clock, with a snapshot and some
    /// fluid, a run starts on what can be restored now, at a share of the cost each tick.
    fn start(&mut self, cfg: &ParadoxSettings, machine: &mut impl Machine) {
        if !(self.redstone.is_active() && self.can_run()) {
            return;
        }
        if !machine.has_snapshot() || self.fluid <= 0 {
            return;
        }
        if machine.paradox_exists() || self.running {
            return;
        }
        let (blocks, entities) = machine.plan(self.target);
        if blocks == 0 && entities == 0 {
            machine.clear();
            return;
        }
        self.planned = (blocks, entities);
        self.running = true;
        let run_time = self.run_time();
        self.fe_per_tick = (f64::from(energy_cost(cfg, blocks, entities)) / f64::from(run_time)).floor() as i32;
        self.fluid_per_tick = (f64::from(fluid_cost(cfg, blocks, entities)) / f64::from(run_time)).floor() as i32;
        machine.sound(Sound::Running);
    }

    /// `stopRunning`: a run that ran its time pays the rest of its cost and restores; one that ran dry fails.
    fn stop(&mut self, cfg: &ParadoxSettings, success: bool, machine: &mut impl Machine) {
        let (blocks, entities) = self.planned;
        if success {
            let run_time = self.run_time();
            let fluid = fluid_cost(cfg, blocks, entities) - self.fluid_per_tick * run_time;
            let fe = energy_cost(cfg, blocks, entities) - self.fe_per_tick * run_time;
            if self.extract_fluid(fluid) == fluid && self.extract_energy(fe) == fe {
                machine.restore();
                self.add_paradox_energy(cfg, cfg.energy_per_block as f32 * blocks as f32);
                self.add_paradox_energy(cfg, cfg.energy_per_entity as f32 * entities as f32);
                machine.sound(Sound::Restored);
            } else {
                machine.sound(Sound::Failed);
            }
        } else {
            machine.sound(Sound::Failed);
        }
        self.time_running = 0;
        self.running = false;
        self.planned = (0, 0);
        machine.clear();
        self.fe_per_tick = 0;
        self.fluid_per_tick = 0;
    }

    /// `addParadoxEnergy`.
    fn add_paradox_energy(&mut self, cfg: &ParadoxSettings, amount: f32) {
        self.paradox_energy = (self.paradox_energy + amount).min(cfg.energy_max as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_area_is_javas_box() {
        let mut a = Area::facing([0, 1, 0]);
        assert_eq!(a.bounds([10, 64, -3]), ([10.0, 65.0, -3.0], [11.0, 66.0, -2.0]));
        assert_eq!(a.blocks([10, 64, -3]), vec![[10, 65, -3]]);
        a.step_radius(0, true);
        a.step_radius(0, true);
        a.step_radius(2, true);
        assert_eq!(a.radius, [1.0, 0.0, 0.5]);
        assert_eq!(a.bounds([0, 0, 0]), ([-1.0, 1.0, 0.0], [2.0, 2.0, 2.0]), "a half radius moves the box half a block");
        assert_eq!(a.blocks([0, 0, 0]).len(), 3 * 2, "x -1..=1, z 0..=1");
        for _ in 0..20 {
            a.step_radius(1, true);
            a.step_offset(2, false);
        }
        assert_eq!((a.radius[1], a.offset[2]), (MAX_RADIUS, -MAX_OFFSET));
        a.step_radius(0, false);
        a.step_radius(0, false);
        a.step_radius(0, false);
        assert_eq!(a.radius[0], 0.0);
    }

    #[derive(Default)]
    struct World {
        snapshot: bool,
        missing: (usize, usize),
        restored: usize,
        cleared: usize,
        sounds: Vec<Sound>,
    }

    impl Machine for World {
        fn has_snapshot(&self) -> bool {
            self.snapshot
        }
        fn plan(&mut self, target: Target) -> (usize, usize) {
            (if target.blocks() { self.missing.0 } else { 0 }, if target.entities() { self.missing.1 } else { 0 })
        }
        fn restore(&mut self) {
            self.restored += 1;
        }
        fn clear(&mut self) {
            self.cleared += 1;
        }
        fn sound(&mut self, sound: Sound) {
            self.sounds.push(sound);
        }
    }

    fn machine() -> (Paradox, ParadoxSettings) {
        let cfg = ParadoxSettings::default();
        let mut p = Paradox::new([0, 0, -1]);
        p.energy = cfg.rf_capacity;
        p.fluid = 1000;
        (p, cfg)
    }

    #[test]
    fn a_pulse_restores_two_blocks_in_twenty_ticks() {
        let (mut p, cfg) = machine();
        let mut w = World { snapshot: true, missing: (2, 0), ..World::default() };
        p.tick(&cfg, false, &mut w);
        assert!(!p.running, "pulse mode waits for a rising edge");
        p.tick(&cfg, true, &mut w);
        assert!(p.running);
        assert_eq!(p.run_time(), 20);
        assert_eq!((p.fe_per_tick, p.fluid_per_tick), (25_000, 5));
        assert_eq!(w.sounds, vec![Sound::Running, Sound::Running], "the start, and tick 0 of the run");
        for _ in 0..19 {
            p.tick(&cfg, true, &mut w);
        }
        assert_eq!(w.restored, 1, "the twentieth tick of the run restores");
        assert!(!p.running);
        assert_eq!(p.fluid, 1000 - 100);
        assert_eq!(p.energy, cfg.rf_capacity - 500_000);
        assert_eq!(p.paradox_energy, 0.5);
        assert_eq!(w.sounds.last(), Some(&Sound::Restored));
        p.tick(&cfg, true, &mut w);
        assert!(!p.running, "a held signal starts no second run");
    }

    #[test]
    fn a_run_without_energy_fails_and_nothing_to_restore_starts_nothing() {
        let (mut p, cfg) = machine();
        p.energy = 30_000;
        let mut w = World { snapshot: true, missing: (2, 0), ..World::default() };
        p.tick(&cfg, true, &mut w);
        assert!(p.running);
        p.tick(&cfg, true, &mut w);
        assert!(!p.running, "the second tick finds 5,000 FE for 25,000");
        assert_eq!(w.restored, 0);
        assert_eq!(w.sounds.last(), Some(&Sound::Failed));
        assert_eq!(p.fluid, 1000 - 10, "the fluid of both ticks is gone");
        let (mut p, cfg) = machine();
        let mut w = World { snapshot: true, missing: (0, 0), ..World::default() };
        p.tick(&cfg, true, &mut w);
        assert!(!p.running && w.sounds.is_empty());
        let mut w = World { snapshot: false, missing: (3, 0), ..World::default() };
        p.tick(&cfg, false, &mut w);
        p.tick(&cfg, true, &mut w);
        assert!(!p.running, "no snapshot, no run");
        let (mut p, cfg) = machine();
        p.fluid = 0;
        let mut w = World { snapshot: true, missing: (1, 0), ..World::default() };
        p.tick(&cfg, true, &mut w);
        assert!(!p.running, "an empty tank, no run");
    }

    #[test]
    fn the_target_and_the_redstone_modes() {
        let (mut p, cfg) = machine();
        p.target = Target::Entities;
        let mut w = World { snapshot: true, missing: (3, 1), ..World::default() };
        p.tick(&cfg, true, &mut w);
        assert_eq!((p.planned, p.run_time()), ((0, 1), 10));
        let (mut p, cfg) = machine();
        p.redstone.mode = Mode::Ignored;
        let mut w = World { snapshot: true, missing: (1, 0), ..World::default() };
        p.tick(&cfg, false, &mut w);
        assert!(!p.running, "outside pulse mode it waits for its clock");
        for _ in 0..19 {
            p.tick(&cfg, false, &mut w);
        }
        assert!(p.running, "the clock comes round on the twentieth tick");
        assert_eq!(Target::from_index(2), Target::Entities);
        assert_eq!(energy_cost(&cfg, 2, 1), 750_000);
        assert_eq!(fluid_cost(&cfg, 2, 1), 150);
    }

    #[test]
    fn the_buffer_and_the_tank_fill_to_their_capacity() {
        let cfg = ParadoxSettings::default();
        let mut p = Paradox::new([1, 0, 0]);
        assert_eq!(p.fill(&cfg, 1000, true), 1000);
        assert_eq!(p.fluid, 0);
        assert_eq!(p.fill(&cfg, 20_000, false), 16_000);
        assert_eq!(p.drain(500, false), 500);
        assert_eq!(p.receive_energy(&cfg, i64::MAX, false), 10_000_000);
        assert_eq!(p.receive_energy(&cfg, 5, false), 0);
        assert_eq!(p.redstone.mode, Mode::Pulse);
        assert_eq!(p.area.offset, [1, 0, 0]);
    }
}
