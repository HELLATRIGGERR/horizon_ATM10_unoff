//! Mekanism's lasers: the Laser, the Laser Amplifier and the Laser Tractor Beam (`TileEntityBasicLaser`,
//! `TileEntityLaser`, `TileEntityLaserReceptor`, `TileEntityLaserAmplifier`, `TileEntityLaserTractorBeam`,
//! `LaserEnergyContainer`; Mekanism 10.7.19, MIT, Copyright (c) 2017-2025 Aidan C. Brady).
//!
//! Each fires a beam out of its front every tick it has Joules to fire: the Laser its `machine-usage.toml` `laser`
//! (50 000 J in ATM10) or what it holds below that, the amplifier up to its max threshold once its delay passed and
//! it holds its min threshold, the tractor beam all it holds. The beam runs `[laser] range` blocks from 0.501 past
//! the block's centre and stops at the first block whose outline it meets. A receptor there (an amplifier, a
//! tractor beam) takes the Joules into its store, what doesn't fit is lost. Any other block gathers the Joules as
//! digging progress (kept while the beam stays on it; not saved; the Laser drops it on a tick without fire) and breaks once the
//! progress reaches its hardness times `energyPerHardness` (stone 1.5 x 100 000 J), the progress starting over
//! either way; unbreakable blocks gather nothing. The tractor beam keeps the drops of what it breaks.

use crate::sides::{Direction, RelativeSide};
use std::path::Path;

/// The three blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LaserKind {
    Laser,
    Amplifier,
    TractorBeam,
}

impl LaserKind {
    pub const ALL: [Self; 3] = [Self::Laser, Self::Amplifier, Self::TractorBeam];

    pub fn block(self) -> &'static str {
        match self {
            Self::Laser => "mekanism:laser",
            Self::Amplifier => "mekanism:laser_amplifier",
            Self::TractorBeam => "mekanism:laser_tractor_beam",
        }
    }

    pub fn of_block(block: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.block() == block)
    }

    /// Whether it has an active state (`Attributes.ACTIVE`): the Laser's block has `active`, the receptors' none.
    pub fn activates(self) -> bool {
        self == Self::Laser
    }

    /// `machine-storage.toml`'s key.
    pub fn config_key(self) -> &'static str {
        match self {
            Self::Laser => "laser",
            Self::Amplifier => "laserAmplifier",
            Self::TractorBeam => "laserTractorBeam",
        }
    }

    /// `ILaserReceptor` (`TileEntityLaserReceptor`, `canLasersDig` false): a beam on it hands it its Joules.
    pub fn receives(self) -> bool {
        self != Self::Laser
    }

    /// Whether cables, cubes and Forge Energy put Joules in through `side`: the Laser through its back
    /// (`addContainer(.., RelativeSide.BACK)`, insertion `alwaysTrue`); the receptors through none (`internalOnly`:
    /// only beams fill them).
    pub fn takes_energy(self, facing: Direction, side: Direction) -> bool {
        self == Self::Laser && RelativeSide::from_directions(facing, side) == RelativeSide::Back
    }

    /// Whether Joules can be drawn out of it from outside: the amplifier's extraction is `alwaysTrue` on every side,
    /// the Laser's and tractor beam's `notExternal`.
    pub fn gives_energy(self) -> bool {
        self == Self::Amplifier
    }

    /// Its inventory: the tractor beam's 27 output slots (`OutputInventorySlot`, 9 by 3).
    pub fn slots(self) -> usize {
        if self == Self::TractorBeam { TRACTOR_SLOTS } else { 0 }
    }
}

pub const TRACTOR_SLOTS: usize = 27;

/// The `[laser]` section of `general.toml`, `aestheticWorldDamage`, `machine-usage.toml` `laser` and the three stores
/// of `machine-storage.toml`. The defaults are ATM10's files'.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaserConfig {
    pub usage: u64,
    pub storage: [u64; 3],
    pub range: i32,
    pub energy_per_hardness: u64,
    pub energy_per_damage: u64,
    pub aesthetic_world_damage: bool,
}

impl Default for LaserConfig {
    fn default() -> Self {
        Self {
            usage: 50_000,
            storage: [10_000_000, 5_000_000_000, 5_000_000_000],
            range: 64,
            energy_per_hardness: 100_000,
            energy_per_damage: 2_500,
            aesthetic_world_damage: true,
        }
    }
}

fn long(table: Option<&toml::Table>, key: &str, min: i64) -> Option<u64> {
    table.and_then(|t| t.get(key)).and_then(toml::Value::as_integer).filter(|v| *v >= min).map(|v| v as u64)
}

impl LaserConfig {
    pub fn from_tables(general: Option<&toml::Table>, usage: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let laser = general.and_then(|g| g.get("laser")).and_then(toml::Value::as_table);
        if let Some(v) = long(laser, "range", 1).filter(|v| *v <= 1024) {
            c.range = v as i32;
        }
        if let Some(v) = long(laser, "energyPerHardness", 0) {
            c.energy_per_hardness = v;
        }
        if let Some(v) = long(laser, "energyPerDamage", 1) {
            c.energy_per_damage = v;
        }
        if let Some(v) = general.and_then(|g| g.get("aestheticWorldDamage")).and_then(toml::Value::as_bool) {
            c.aesthetic_world_damage = v;
        }
        if let Some(v) = long(usage, "laser", 0) {
            c.usage = v;
        }
        for (i, kind) in LaserKind::ALL.into_iter().enumerate() {
            if let Some(v) = long(storage, kind.config_key(), 1) {
                c.storage[i] = v;
            }
        }
        c
    }

    pub fn load(config: &Path) -> Self {
        let read = |f: &str| std::fs::read_to_string(config.join("Mekanism").join(f)).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tables(read("general.toml").as_ref(), read("machine-usage.toml").as_ref(), read("machine-storage.toml").as_ref())
    }

    pub fn capacity(&self, kind: LaserKind) -> u64 {
        self.storage[kind as usize]
    }
}

/// `TileEntityLaserAmplifier.RedstoneOutput`, saved as its ordinal (`output_mode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedstoneOutput {
    #[default]
    Off,
    EntityDetection,
    EnergyContents,
}

impl RedstoneOutput {
    pub const ALL: [Self; 3] = [Self::Off, Self::EntityDetection, Self::EnergyContents];

    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(3) as usize]
    }

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn next(self) -> Self {
        Self::by_id(self.id() + 1)
    }

    pub fn previous(self) -> Self {
        Self::by_id(self.id() + 2)
    }

    /// Its lang entry's English text (`LaserAmplifier.RedstoneOutput`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::EntityDetection => "Entity Detection",
            Self::EnergyContents => "Energy Contents",
        }
    }
}

/// `IRedstoneControl.RedstoneControl`, saved as its ordinal (`control_type`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedstoneControl {
    #[default]
    Disabled,
    High,
    Low,
    Pulse,
}

impl RedstoneControl {
    pub fn by_id(id: i32) -> Self {
        [Self::Disabled, Self::High, Self::Low, Self::Pulse][id.rem_euclid(4) as usize]
    }

    /// `TileEntityMekanism.canFunction`: `powered` now, `was` on the tick before.
    pub fn allows(self, powered: bool, was: bool) -> bool {
        match self {
            Self::Disabled => true,
            Self::High => powered,
            Self::Low => !powered,
            Self::Pulse => powered && !was,
        }
    }
}

/// The amplifier's settings (`min`, `max`, `time`, `output_mode`, `control_type`) and its delay's count (`ticks`,
/// not saved).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amplifier {
    pub min: u64,
    pub max: u64,
    pub delay: i32,
    pub ticks: i32,
    pub output: RedstoneOutput,
    pub control: RedstoneControl,
}

impl Amplifier {
    /// A new amplifier's: min 0, max its store, no delay, no output, redstone ignored.
    pub fn new(capacity: u64) -> Self {
        Self { min: 0, max: capacity, delay: 0, ticks: 0, output: RedstoneOutput::Off, control: RedstoneControl::Disabled }
    }

    /// `updateMinThreshold`: capped at the store; a min past the max raises the max. Whether it changed.
    pub fn set_min(&mut self, value: u64, capacity: u64) -> bool {
        let v = value.min(capacity);
        if v == self.min {
            return false;
        }
        self.min = v;
        if self.min > self.max {
            self.max = self.min;
        }
        true
    }

    /// `updateMaxThreshold`: capped at the store; a max under the min lowers the min.
    pub fn set_max(&mut self, value: u64, capacity: u64) -> bool {
        let v = value.min(capacity);
        if v == self.max {
            return false;
        }
        self.max = v;
        if self.max < self.min {
            self.min = self.max;
        }
        true
    }

    /// `setDelay`: never below 0.
    pub fn set_delay(&mut self, delay: i32) -> bool {
        let d = delay.max(0);
        let changed = d != self.delay;
        self.delay = d;
        changed
    }

    /// The start of its `onUpdateServer`, then `toFire`: the delay's count goes up to the delay and back to 0, and it
    /// fires up to its max once the count reached the delay, it holds its min and redstone lets it.
    pub fn to_fire(&mut self, energy: u64, can_function: bool) -> u64 {
        if self.ticks < self.delay {
            self.ticks += 1;
        } else {
            self.ticks = 0;
        }
        if self.ticks >= self.delay && energy >= self.min && can_function { self.max } else { 0 }
    }

    /// `getRedstoneLevel`: its store's fill in `ENERGY_CONTENTS`, 15 while entities are in its beam in
    /// `ENTITY_DETECTION`, else 0.
    pub fn redstone_level(&self, energy: u64, capacity: u64, entities: bool) -> i32 {
        match self.output {
            RedstoneOutput::EnergyContents => crate::dynamic_tank::redstone_level(energy.min(i64::MAX as u64) as i64, capacity.min(i64::MAX as u64) as i64),
            RedstoneOutput::EntityDetection if entities => 15,
            _ => 0,
        }
    }
}

/// What the beam meets, from the level.
pub trait Beam {
    /// `Level.clip` from `from` to `to` (block outlines, no fluids): the block met and the point.
    fn clip(&mut self, from: [f64; 3], to: [f64; 3]) -> Option<([i32; 3], [f64; 3])>;
    /// Whether a laser receptor that lasers can't dig is at `pos` (`Capabilities.LASER_RECEPTOR`).
    fn receptor(&mut self, pos: [i32; 3]) -> bool;
    /// `receiveLaserEnergy`: hands the receptor at `pos` `joules`.
    fn give(&mut self, pos: [i32; 3], joules: u64);
    /// `getDestroySpeed`: None when it can't be read.
    fn hardness(&mut self, pos: [i32; 3]) -> Option<f32>;
    /// The break (`withFakePlayer`, `handleBreakBlock`).
    fn break_block(&mut self, pos: [i32; 3]);
}

/// A laser's saved Joules, `last_fired` and `active_state`, and its digging (not saved). `activates`: whether it
/// has an active state (`LaserKind::activates`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Laser {
    pub energy: u64,
    pub last_fired: u64,
    pub activates: bool,
    pub active: bool,
    pub digging: Option<[i32; 3]>,
    pub progress: u64,
}

/// What a tick's beam did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fired {
    pub firing: u64,
    /// From the beam's start to where it stopped.
    pub length: f64,
    pub hit: Option<[i32; 3]>,
    pub handed: u64,
    pub broke: bool,
}

/// The beam's start (`Pos3D.create(this).centre().translate(direction, 0.501)`) and end (`laserRange - 0.002` on).
pub fn beam(pos: [i32; 3], facing: Direction, range: i32) -> ([f64; 3], [f64; 3]) {
    let n = facing.offset();
    let from: [f64; 3] = std::array::from_fn(|i| f64::from(pos[i]) + 0.5 + 0.501 * f64::from(n[i]));
    let reach = f64::from(range) - 0.002;
    (from, std::array::from_fn(|i| from[i] + reach * f64::from(n[i])))
}

impl Laser {
    /// `TileEntityBasicLaser.onUpdateServer` with `to_fire` from the kind (`toFire`): it fires what it holds up to
    /// that. Entities in the beam aren't looked at (not ported): the beam keeps all its Joules for the block.
    pub fn tick(&mut self, pos: [i32; 3], facing: Direction, to_fire: u64, config: &LaserConfig, beam: &mut impl Beam) -> Fired {
        let firing = self.energy.min(to_fire);
        if firing == 0 {
            // A Laser that stops goes inactive at once (`getActive` is `currentActive`, whatever the block's state
            // shows) and drops its progress and `last_fired` (the target stays). The receptors are never active: they
            // keep both.
            if self.active {
                self.active = false;
                self.progress = 0;
                self.last_fired = 0;
            }
            return Fired::default();
        }
        self.active = self.activates;
        self.last_fired = firing;
        let (from, end) = self::beam(pos, facing, config.range);
        let hit = beam.clip(from, end);
        let to = hit.map_or(end, |(_, at)| at);
        let length = (0..3).map(|i| (to[i] - from[i]).powi(2)).sum::<f64>().sqrt();
        let mut fired = Fired { firing, length, hit: hit.map(|(p, _)| p), ..Fired::default() };
        let remaining = firing;
        match hit {
            None => {
                self.digging = None;
                self.progress = 0;
            }
            Some((at, _)) => {
                if self.digging != Some(at) {
                    self.digging = Some(at);
                    self.progress = 0;
                }
                if beam.receptor(at) {
                    beam.give(at, remaining);
                    fired.handed = remaining;
                } else if let Some(hardness) = beam.hardness(at).filter(|h| *h >= 0.0) {
                    self.progress = self.progress.saturating_add(remaining);
                    if self.progress as f32 >= hardness * config.energy_per_hardness as f32 {
                        if config.aesthetic_world_damage {
                            beam.break_block(at);
                            fired.broke = true;
                        }
                        self.progress = 0;
                    }
                }
            }
        }
        self.energy -= firing;
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        blocks: Vec<([i32; 3], f32, bool)>,
        broken: Vec<[i32; 3]>,
        given: Vec<([i32; 3], u64)>,
    }

    impl Beam for World {
        fn clip(&mut self, from: [f64; 3], to: [f64; 3]) -> Option<([i32; 3], [f64; 3])> {
            // Axis beams only: the nearest block on the segment.
            let mut best: Option<(f64, [i32; 3])> = None;
            for (p, _, _) in &self.blocks {
                let centre: [f64; 3] = std::array::from_fn(|i| f64::from(p[i]) + 0.5);
                let d: [f64; 3] = std::array::from_fn(|i| to[i] - from[i]);
                let len = d.iter().map(|v| v * v).sum::<f64>().sqrt();
                let t = (0..3).map(|i| (centre[i] - from[i]) * d[i] / len).sum::<f64>() - 0.5;
                let off = (0..3).map(|i| (centre[i] - from[i] - d[i] / len * (t + 0.5)).powi(2)).sum::<f64>();
                if off < 0.01 && t >= -0.5 && t <= len && best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, *p));
                }
            }
            best.map(|(t, p)| {
                let d: [f64; 3] = std::array::from_fn(|i| to[i] - from[i]);
                let len = d.iter().map(|v| v * v).sum::<f64>().sqrt();
                (p, std::array::from_fn(|i| from[i] + d[i] / len * t.max(0.0)))
            })
        }
        fn receptor(&mut self, pos: [i32; 3]) -> bool {
            self.blocks.iter().any(|(p, _, r)| *p == pos && *r)
        }
        fn give(&mut self, pos: [i32; 3], joules: u64) {
            self.given.push((pos, joules));
        }
        fn hardness(&mut self, pos: [i32; 3]) -> Option<f32> {
            self.blocks.iter().find(|(p, _, _)| *p == pos).map(|(_, h, _)| *h)
        }
        fn break_block(&mut self, pos: [i32; 3]) {
            self.broken.push(pos);
            self.blocks.retain(|(p, _, _)| *p != pos);
        }
    }

    fn world(blocks: &[([i32; 3], f32, bool)]) -> World {
        World { blocks: blocks.to_vec(), broken: vec![], given: vec![] }
    }

    #[test]
    fn the_beam_starts_past_the_front_face_and_runs_the_range() {
        let (from, to) = beam([0, 64, 0], Direction::North, 64);
        assert_eq!(from, [0.5, 64.5, 0.5 - 0.501]);
        assert!((to[2] - (0.5 - 0.501 - 63.998)).abs() < 1e-9);
    }

    #[test]
    fn stone_breaks_on_the_third_tick_of_50_000_joules_and_obsidian_after_100() {
        let c = LaserConfig::default();
        let mut w = world(&[([0, 0, -3], 1.5, false)]);
        let mut l = Laser { energy: 1_000_000, ..Laser::default() };
        let mut out = vec![];
        for _ in 0..3 {
            let f = l.tick([0, 0, 0], Direction::North, c.usage, &c, &mut w);
            out.push((f.firing, l.progress, f.broke));
        }
        assert_eq!(out, [(50_000, 50_000, false), (50_000, 100_000, false), (50_000, 0, true)]);
        assert_eq!((l.energy, l.last_fired, w.broken.clone()), (850_000, 50_000, vec![[0, 0, -3]]));
        let f = l.tick([0, 0, 0], Direction::North, c.usage, &c, &mut w);
        assert_eq!((f.hit, l.digging, l.progress), (None, None, 0), "nothing left in range");

        let mut w = world(&[([0, 0, 5], 50.0, false)]);
        let mut l = Laser { energy: 10_000_000, ..Laser::default() };
        let ticks = (1..=200).find(|_| l.tick([0, 0, 0], Direction::South, c.usage, &c, &mut w).broke);
        assert_eq!(ticks, Some(100));
    }

    #[test]
    fn short_joules_fire_what_is_left_and_a_tick_without_fire_drops_the_progress() {
        let c = LaserConfig::default();
        let mut w = world(&[([0, 2, 0], 1.5, false)]);
        let mut l = Laser { energy: 120_000, activates: true, ..Laser::default() };
        let fired: Vec<(u64, u64)> = (0..3).map(|_| (l.tick([0, 0, 0], Direction::Up, c.usage, &c, &mut w).firing, l.last_fired)).collect();
        assert_eq!(fired, [(50_000, 50_000), (50_000, 50_000), (20_000, 20_000)]);
        assert_eq!((l.progress, l.active), (120_000, true));
        l.tick([0, 0, 0], Direction::Up, c.usage, &c, &mut w);
        assert_eq!((l.progress, l.last_fired, l.active, l.digging), (0, 0, false, Some([0, 2, 0])), "the idle tick");
        l.energy = 30_000;
        assert!(!l.tick([0, 0, 0], Direction::Up, c.usage, &c, &mut w).broke, "30 000 J more don't break it");
        assert_eq!((l.progress, l.last_fired), (30_000, 30_000));
        // A receptor keeps both across the tick without fire.
        let mut w = world(&[([0, 2, 0], 1.5, false)]);
        let mut r = Laser { energy: 120_000, ..Laser::default() };
        for _ in 0..4 {
            r.tick([0, 0, 0], Direction::Up, c.usage, &c, &mut w);
        }
        assert_eq!((r.progress, r.last_fired, r.active), (120_000, 20_000, false));
        r.energy = 30_000;
        assert!(r.tick([0, 0, 0], Direction::Up, c.usage, &c, &mut w).broke);
    }

    #[test]
    fn unbreakable_blocks_and_aesthetic_damage_off_gather_nothing_or_break_nothing() {
        let c = LaserConfig::default();
        let mut w = world(&[([1, 0, 0], -1.0, false)]);
        let mut l = Laser { energy: 1_000_000, ..Laser::default() };
        l.tick([0, 0, 0], Direction::East, c.usage, &c, &mut w);
        assert_eq!((l.digging, l.progress, l.energy), (Some([1, 0, 0]), 0, 950_000));
        let off = LaserConfig { aesthetic_world_damage: false, ..c };
        let mut w = world(&[([1, 0, 0], 0.5, false)]);
        let f = l.tick([0, 0, 0], Direction::East, off.usage, &off, &mut w);
        assert_eq!((f.broke, l.progress, w.broken.len()), (false, 0, 0), "the progress resets, the block stays");
    }

    #[test]
    fn a_receptor_takes_the_joules_and_a_new_target_starts_over() {
        let c = LaserConfig::default();
        let mut w = world(&[([0, 0, -4], 3.5, true)]);
        let mut l = Laser { energy: 60_000, digging: Some([9, 9, 9]), progress: 70_000, ..Laser::default() };
        let f = l.tick([0, 0, 0], Direction::North, c.usage, &c, &mut w);
        assert_eq!((f.handed, w.given.clone(), l.digging, l.progress), (50_000, vec![([0, 0, -4], 50_000)], Some([0, 0, -4]), 0));
    }

    #[test]
    fn the_amplifier_waits_its_delay_its_min_and_redstone_then_fires_up_to_its_max() {
        let cap = 5_000_000_000;
        let mut a = Amplifier::new(cap);
        assert_eq!(a.to_fire(0, true), cap, "no delay: every tick");
        a.set_delay(2);
        let fires: Vec<bool> = (0..6).map(|_| a.to_fire(10, true) > 0).collect();
        assert_eq!(fires, [false, true, false, false, true, false], "a delay of 2 fires one tick in 3");
        let mut a = Amplifier::new(cap);
        a.set_min(1_000, cap);
        assert_eq!((a.to_fire(999, true), a.to_fire(1_000, true), a.to_fire(1_000, false)), (0, cap, 0));
        assert!(a.set_max(500, cap));
        assert_eq!((a.min, a.max), (500, 500), "a max under the min lowers it");
        assert!(a.set_min(9_000_000_000, cap));
        assert_eq!((a.min, a.max), (cap, cap), "capped at the store, the max raised");
        assert!(!a.set_delay(-3) && a.delay == 0, "never below 0");
    }

    #[test]
    fn redstone_output_and_control() {
        let mut a = Amplifier::new(100);
        assert_eq!(a.redstone_level(50, 100, true), 0);
        a.output = RedstoneOutput::EnergyContents;
        assert_eq!((a.redstone_level(0, 100, false), a.redstone_level(50, 100, false), a.redstone_level(100, 100, false)), (0, 8, 15));
        a.output = RedstoneOutput::EntityDetection;
        assert_eq!((a.redstone_level(50, 100, true), a.redstone_level(50, 100, false)), (15, 0));
        assert_eq!((RedstoneOutput::Off.next(), RedstoneOutput::Off.previous()), (RedstoneOutput::EntityDetection, RedstoneOutput::EnergyContents));
        use RedstoneControl::*;
        assert_eq!([Disabled, High, Low, Pulse].map(|c| c.allows(true, true)), [true, true, false, false]);
        assert!(Pulse.allows(true, false) && !Low.allows(true, false));
    }

    #[test]
    fn energy_sides_and_config() {
        use Direction::*;
        assert!(LaserKind::Laser.takes_energy(North, South) && !LaserKind::Laser.takes_energy(North, North));
        assert!(LaserKind::Laser.takes_energy(Up, Down));
        assert!(Direction::ALL.iter().all(|d| !LaserKind::Amplifier.takes_energy(North, *d) && !LaserKind::TractorBeam.takes_energy(North, *d)));
        assert!(LaserKind::Amplifier.gives_energy() && !LaserKind::TractorBeam.gives_energy());
        assert_eq!(LaserKind::of_block("mekanism:laser_tractor_beam").map(|k| k.slots()), Some(27));
        let general: toml::Table = "aestheticWorldDamage = false\n[laser]\nrange = 32\nenergyPerHardness = 1000\nenergyPerDamage = 0\n".parse().unwrap();
        let usage: toml::Table = "laser = 7".parse().unwrap();
        let storage: toml::Table = "laserAmplifier = 9\nlaser = 0".parse().unwrap();
        let c = LaserConfig::from_tables(Some(&general), Some(&usage), Some(&storage));
        assert_eq!((c.range, c.energy_per_hardness, c.energy_per_damage, c.aesthetic_world_damage, c.usage), (32, 1000, 2500, false, 7));
        assert_eq!(c.storage, [10_000_000, 9, 5_000_000_000]);
    }
}
