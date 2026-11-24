//! What every T2 machine adds to its T1 (`PoweredMachineBE`, `AreaAffectingBE`, `FilterableBE`): a 100,000 FE buffer
//! that takes Forge Energy from any side and pays for each operation, an area (`AreaAffectingData`: the block in front
//! by default, then a radius and an offset on each axis) and nine filter slots with the allowlist and compare-components
//! settings. The energy bar on the screen reads the buffer.
use crate::{
    machines::{Clock, Filter},
    paradox::Area,
    redstone::Mode,
};

/// `PoweredMachineBE.getMaxEnergy`.
pub const CAPACITY: i32 = 100_000;
/// `BlockPlacerT2BE.getStandardEnergyCost`: a placement.
pub const PLACER_COST: i32 = 500;
/// `BlockBreakerT2BE.getStandardEnergyCost`: a block broken.
pub const BREAKER_COST: i32 = 500;

/// A T2 machine's buffer (`MachineEnergyStorage`, saved as `energy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Powered {
    pub energy: i32,
}

impl Powered {
    /// `hasEnoughPower`: `cost` can be taken.
    pub fn has(&self, cost: i32) -> bool {
        self.energy >= cost
    }

    /// `extractEnergy(cost, false)`: what it took.
    pub fn take(&mut self, cost: i32) -> i32 {
        let took = cost.clamp(0, self.energy);
        self.energy -= took;
        took
    }

    /// `receiveEnergy`: what of `amount` fits (with `simulate`, would fit).
    pub fn receive(&mut self, amount: i64, simulate: bool) -> i64 {
        let took = amount.clamp(0, i64::from(CAPACITY - self.energy));
        if !simulate {
            self.energy += took as i32;
        }
        took
    }

    /// The energy bar's rows of 70 (`PoweredMachineContainerData`'s energy over the buffer).
    pub fn rows(&self) -> i32 {
        (i64::from(self.energy) * 70 / i64::from(CAPACITY)) as i32
    }
}

/// The settings every T2 machine shares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shared {
    pub powered: Powered,
    pub area: Area,
    pub filter: Filter,
}

impl Shared {
    /// A new T2 machine facing `facing` (a side's normal): an empty buffer, the block in front, an empty denylist.
    pub fn new(facing: [i32; 3]) -> Self {
        Self { powered: Powered::default(), area: Area::facing(facing), filter: Filter::default() }
    }

    /// The area's positions nearest the machine first (`sorted(comparingDouble(distSqr))`, a stable sort of
    /// `betweenClosed`'s order).
    pub fn nearest_first(&self, pos: [i32; 3]) -> Vec<[i32; 3]> {
        let mut spots = self.area.blocks(pos);
        let dist = |p: &[i32; 3]| (0..3).map(|a| i64::from(p[a] - pos[a]).pow(2)).sum::<i64>();
        spots.sort_by_key(dist);
        spots
    }

    /// The shared buttons: 9 render area, 10 to 15 an axis's radius up or down, 16 to 21 its offset, 24 allowlist,
    /// 25 compare components. True when `id` was one of them.
    pub fn button(&mut self, id: i32) -> bool {
        match id {
            9 => self.area.render = !self.area.render,
            10..=15 => self.area.step_radius(((id - 10) / 2) as usize, (id - 10) % 2 == 0),
            16..=21 => self.area.step_offset(((id - 16) / 2) as usize, (id - 16) % 2 == 0),
            24 => self.filter.allowlist = !self.filter.allowlist,
            25 => self.filter.compare_nbt = !self.filter.compare_nbt,
            _ => return false,
        }
        true
    }
}

/// A T2 machine's places to work on (`positionsToPlace`): found together on a run, nearest first, one used each run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queue<P> {
    pub clock: Clock,
    pub spots: Vec<P>,
}

impl<P> Default for Queue<P> {
    fn default() -> Self {
        Self { clock: Clock::default(), spots: Vec::new() }
    }
}

impl<P> Queue<P> {
    /// One server tick of the Placer T2 (`tickServer`, `doBlockPlace`): `valid_stack` the slot holds a block item
    /// (`isStackValid`), `powered` the buffer has a placement's energy (`canPlace`); `find` finds the places when none
    /// is queued. The place to work on this tick.
    pub fn tick(&mut self, valid_stack: bool, powered: bool, signal: bool, find: impl FnOnce() -> Vec<P>) -> Option<P> {
        self.clock.tick(signal);
        if !valid_stack {
            self.clock.redstone.pulsed = false;
            return None;
        }
        // `clearTrackerIfNeeded` (a pulse is used up by asking).
        if !self.spots.is_empty() && (!powered || (!self.clock.redstone.is_active() && self.clock.redstone.mode != Mode::Pulse)) {
            self.spots.clear();
            return None;
        }
        if !powered {
            return None;
        }
        if self.clock.redstone.is_active() && self.clock.can_run() && self.spots.is_empty() {
            self.spots = find();
        }
        if self.spots.is_empty() || !self.clock.can_run() {
            return None;
        }
        Some(self.spots.remove(0))
    }
}

/// The blocks of a Breaker T2's area, as its tick sees them.
pub trait Blocks<P> {
    /// The block state at `at` now; none for air.
    fn state(&mut self, at: &P) -> Option<u32>;
    /// `getDestroyProgress` of the tool on it.
    fn progress(&mut self, at: &P) -> f32;
    /// `sendPackets` for the tracker's `index`th entry (each has its breaker id): the crack stage, -1 removes it.
    fn crack(&mut self, at: &P, index: usize, stage: i32);
    /// `tryBreakBlock`: the energy taken (none: false), then the block broken with the tool, drops and wear.
    fn break_block(&mut self, at: &P) -> bool;
}

/// The Block Breaker T2 (`BlockBreakerT2BE`): on a run it starts tracking every block its area has to mine, nearest
/// first (`blockBreakingTracker`, insertion order), then mines the first one a tick at a time and, once it breaks or
/// changes, takes the next on the following run. `clock` and `sneaking` are saved; the tracker is transient.
#[derive(Clone, Debug, PartialEq)]
pub struct BreakerT2<P> {
    pub clock: Clock,
    pub sneaking: bool,
    pub tracked: Vec<(P, crate::breaker::Tracked)>,
    /// `currentBlock`: the first entry is being mined.
    pub current: bool,
}

impl<P> Default for BreakerT2<P> {
    fn default() -> Self {
        Self { clock: Clock::default(), sneaking: false, tracked: Vec::new(), current: false }
    }
}

impl<P: PartialEq> BreakerT2<P> {
    fn clear(&mut self, blocks: &mut dyn Blocks<P>) {
        for (i, (at, _)) in self.tracked.drain(..).enumerate() {
            blocks.crack(&at, i, -1);
        }
        self.current = false;
    }

    /// One server tick (`tickServer`, `doBlockBreak`). `has_tool`: the slot holds an item; `powered`: a break's
    /// energy is there (`canMine`); `find` the area's valid blocks nearest first (`findBlocksToMine`).
    pub fn tick(&mut self, has_tool: bool, powered: bool, signal: bool, find: impl FnOnce() -> Vec<P>, blocks: &mut dyn Blocks<P>) {
        self.clock.tick(signal);
        // `clearTrackerIfNeeded`: both checks, the redstone one asking (and in pulse mode using) the pulse.
        if !self.tracked.is_empty() {
            if !has_tool {
                self.clear(blocks);
            }
            if !self.clock.redstone.is_active() && self.clock.redstone.mode != Mode::Pulse {
                self.clear(blocks);
            }
        }
        if !has_tool {
            self.clock.redstone.pulsed = false;
            return;
        }
        if self.clock.redstone.is_active() && self.clock.can_run() && self.tracked.is_empty() {
            for at in find() {
                if self.tracked.iter().any(|(p, _)| *p == at) {
                    continue;
                }
                if let Some(state) = blocks.state(&at) {
                    let progress = blocks.progress(&at);
                    self.tracked.push((at, crate::breaker::Tracked { state, ticks: 0, last_sent: -1, progress }));
                }
            }
        }
        if self.tracked.is_empty() || !powered {
            return;
        }
        if !self.current && self.clock.can_run() {
            self.current = true;
        }
        if self.current && self.mine(blocks) {
            let (at, _) = self.tracked.remove(0);
            blocks.crack(&at, 0, -1);
            self.current = false;
        }
    }

    /// `mineBlock` on the first tracked block: true when it is gone (broken, air, or another block now).
    fn mine(&mut self, blocks: &mut dyn Blocks<P>) -> bool {
        let Some((at, t)) = self.tracked.first_mut() else { return true };
        let Some(now) = blocks.state(at) else { return true };
        if now != t.state {
            return true;
        }
        t.ticks += 1;
        let done = t.progress * t.ticks as f32;
        let stage = (done * 10.0) as i32;
        if stage != t.last_sent && stage < 10 {
            blocks.crack(at, 0, stage);
            t.last_sent = stage;
        }
        done >= 1.0 && blocks.break_block(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blocks along a line: each `progress` a tick, broken when asked while there is energy.
    struct Row {
        states: Vec<Option<u32>>,
        energy: i32,
        broken: Vec<(usize, u32)>,
        tick: u32,
    }

    impl Blocks<usize> for Row {
        fn state(&mut self, at: &usize) -> Option<u32> {
            self.states[*at]
        }
        fn progress(&mut self, _: &usize) -> f32 {
            0.5
        }
        fn crack(&mut self, _: &usize, _: usize, _: i32) {}
        fn break_block(&mut self, at: &usize) -> bool {
            if self.energy < BREAKER_COST {
                return false;
            }
            self.energy -= BREAKER_COST;
            self.states[*at] = None;
            self.broken.push((*at, self.tick));
            true
        }
    }

    #[test]
    fn the_breaker_t2_mines_its_area_nearest_first_one_block_a_run() {
        let mut b = BreakerT2::<usize>::default();
        let mut row = Row { states: vec![Some(1), Some(1), None, Some(2)], energy: 1000, broken: Vec::new(), tick: 0 };
        for t in 1..=80 {
            row.tick = t;
            let powered = row.energy >= BREAKER_COST;
            b.tick(true, powered, false, || vec![0, 1, 2, 3], &mut row);
        }
        // Found at tick 20 (air skipped), two ticks a block; the next waits for the next run; then the energy is gone.
        assert_eq!(row.broken, [(0, 21), (1, 41)]);
        assert_eq!(row.energy, 0);
        assert_eq!(b.tracked.len(), 1, "the last block waits for energy");
        // The tool taken out: the tracker is dropped.
        b.tick(false, true, false, Vec::new, &mut row);
        assert!(b.tracked.is_empty());
    }

    #[test]
    fn a_queue_finds_its_places_once_and_uses_one_a_run() {
        let mut q = Queue::<i32>::default();
        let mut found = 0;
        let mut used = Vec::new();
        for _ in 0..60 {
            if let Some(p) = q.tick(true, true, false, || {
                found += 1;
                vec![1, 2]
            }) {
                used.push(p);
            }
        }
        assert_eq!((found, used), (2, vec![1, 2, 1]), "found at ticks 20 and 60, one used every 20");
        // No power: what was queued goes.
        assert!(!q.spots.is_empty());
        assert_eq!(q.tick(true, false, false, Vec::new), None);
        assert!(q.spots.is_empty());
        // No block item: nothing, and a pulse is dropped.
        q.clock.redstone.pulsed = true;
        assert_eq!(q.tick(false, true, false, || vec![9]), None);
        assert!(!q.clock.redstone.pulsed);
    }

    #[test]
    fn the_buffer_takes_and_pays() {
        let mut p = Powered::default();
        assert!(!p.has(250));
        assert_eq!(p.receive(150_000, true), 100_000);
        assert_eq!(p.energy, 0, "simulated");
        assert_eq!(p.receive(1000, false), 1000);
        assert!(p.has(250));
        assert_eq!(p.take(250), 250);
        assert_eq!((p.energy, p.rows()), (750, 0));
        p.energy = CAPACITY;
        assert_eq!((p.receive(5, false), p.rows()), (0, 70));
    }

    #[test]
    fn the_area_is_walked_nearest_first() {
        let mut s = Shared::new([0, 0, 1]);
        assert_eq!(s.nearest_first([0, 64, 0]), [[0, 64, 1]], "the block in front");
        assert!(s.button(10) && s.button(14));
        // Radius 0.5 on x and z: the area moves half a block, and covers two by two.
        let spots = s.nearest_first([0, 64, 0]);
        assert_eq!(spots.len(), 4);
        assert_eq!(spots[0], [0, 64, 1]);
        assert!(s.button(24) && s.filter.allowlist && !s.button(30));
    }
}
