//! The other T2 machines (`DropperT2BE`, `SensorT2BE`, `FluidPlacerT2BE`, `FluidCollectorT2BE`, `BlockSwapperT2BE`) on what every
//! T2 machine shares (`t2`): a Forge Energy buffer that pays for each operation, an area and nine filter slots. The Dropper T2
//! keeps a list of the machine slots it still has to drop across operations; the Sensor T2 pays for the whole area it scans;
//! the fluid machines find their places together as the Placer T2 does and use one a run; the Swapper T2 pays for each block
//! or entity it moves.
use crate::{
    machines::{BUCKET, Clock, Container, Tank},
    more::{Bucket, fill_tank_from},
    redstone::Mode,
    t2::{Powered, Queue},
};

/// `DropperT2BE.getStandardEnergyCost`: a stack dropped.
pub const DROPPER_COST: i32 = 25;
/// `SensorT2BE.getStandardEnergyCost`: a block of the area scanned.
pub const SENSOR_COST: i32 = 2;
/// `FluidPlacerT2BE` and `FluidCollectorT2BE` `getStandardEnergyCost`: a bucket placed or collected.
pub const FLUID_COST: i32 = 500;
/// `BlockSwapperT2BE.getStandardEnergyCost`: a block or entity swapped (a block counts on both sides).
pub const SWAPPER_COST: i32 = 50;
/// The T2 fluid machines' tank (`getMaxMB`).
pub const FLUID_CAPACITY: i32 = 32_000;
/// The Dropper T2's machine slots (`MACHINE_SLOTS = 9`).
pub const DROPPER_SLOTS: usize = 9;

/// `width * height * depth` of an area as the T2 machines read it from its box: the difference of the floors on each
/// axis (`getEnergyCost`, `canSwap`).
pub fn area_volume(min: [f64; 3], max: [f64; 3]) -> i32 {
    (0..3).map(|a| (max[a].floor() - min[a].floor()).abs() as i32).product()
}

/// `extractEnergy(cost, false) >= cost`: the buffer gives what it has (all of it when that is short) and says whether it
/// was enough, as the Sensor T2's `canSense` does.
pub fn pay_all(powered: &mut Powered, cost: i32) -> bool {
    powered.take(cost) >= cost
}

/// `AABB.contains` of a point in both boxes (`isInBothAreas`).
pub fn in_both(a: ([f64; 3], [f64; 3]), b: ([f64; 3], [f64; 3]), p: [f64; 3]) -> bool {
    let inside = |(min, max): ([f64; 3], [f64; 3])| (0..3).all(|i| p[i] >= min[i] && p[i] < max[i]);
    inside(a) && inside(b)
}

// ---- The Dropper T2 --------------------------------------------------------------------------------------------

/// `populateDropSlots` with no filter: the first machine slot that holds an item.
pub fn populate_plain(occupied: &[bool]) -> Vec<usize> {
    occupied.iter().position(|o| *o).into_iter().collect()
}

/// `populateFilteredDropSlots`: for each filter item (in order) the first machine slot with the same item; one filter
/// item with none clears the list. Two filter items alike list their slot twice.
pub fn populate_filtered<K>(filters: &[K], slots: &[Option<K>], same: impl Fn(&K, &K) -> bool) -> Vec<usize> {
    let mut list = Vec::new();
    for f in filters {
        match slots.iter().position(|s| s.as_ref().is_some_and(|s| same(f, s))) {
            Some(slot) => list.push(slot),
            None => return Vec::new(),
        }
    }
    list
}

/// The Dropper T2 (`DropperT2BE`, `DropperT1BE.doDrop`): `list` is the machine slots left to drop (Java's transient
/// `slotsToDropList`), found on a run and dropped one an operation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DropperT2 {
    pub clock: Clock,
    pub list: Vec<usize>,
}

impl DropperT2 {
    /// `isActiveRedstone` of the dropper: asking uses a pulse up, and with nothing listed it forces the clock to 0.
    fn active(&mut self) -> bool {
        let pulse = self.clock.redstone.mode == Mode::Pulse && self.clock.redstone.pulsed;
        let active = self.clock.redstone.is_active();
        if pulse && self.list.is_empty() {
            self.clock.operation_ticks = 0;
        }
        active
    }

    /// One server tick (`tickServer`, `doDrop`). `powered`: the buffer has a drop's energy (`canDrop`); `populate` lists
    /// the slots to drop; `occupied`: the machine slot holds an item. The slot to drop this tick.
    pub fn tick(&mut self, signal: bool, powered: bool, populate: impl FnOnce() -> Vec<usize>, occupied: impl Fn(usize) -> bool) -> Option<usize> {
        self.clock.tick(signal);
        // `clearTrackerIfNeeded`.
        if !self.list.is_empty() && (!powered || (!self.active() && self.clock.redstone.mode != Mode::Pulse)) {
            self.list.clear();
            return None;
        }
        if !powered {
            return None;
        }
        if self.active() && self.clock.can_run() && self.list.is_empty() {
            self.list = populate();
        }
        if self.list.is_empty() || self.clock.operation_ticks != 0 {
            return None;
        }
        let slot = self.list.remove(0);
        if !occupied(slot) {
            self.list.clear();
            return None;
        }
        Some(slot)
    }
}

// ---- The fluid machines ----------------------------------------------------------------------------------------

/// The level around a T2 fluid machine: where it works (found together nearest first), the fluid source or the spot to
/// fill at a place.
pub trait Pools<P> {
    /// `findSpotsToPlace` or `findSpotsToCollect`: the valid places of the area, nearest first.
    fn find(&mut self) -> Vec<P>;
    /// `getLiquidBlockAt`: the fluid of the source block there.
    fn source(&mut self, at: &P) -> Option<String>;
    /// `level.setBlock(pos, AIR, 3)` with the bucket's sound; whether the block changed.
    fn take(&mut self, at: &P) -> bool;
    /// `FluidUtil.tryPlaceFluid` of a bucket of `fluid`; whether it was placed (or evaporated).
    fn place(&mut self, at: &P, fluid: &str) -> bool;
}

/// One server tick of the Fluid Collector T2 (`FluidCollectorT1BE.tickServer` with `canCollect` and `collectFluid` of the
/// T2): the slot's container takes what it can of the tank, then a place of the area is collected, 500 FE each. True when
/// it collected.
pub fn collector_tick<P: Clone, W: Pools<P> + Container + ?Sized>(q: &mut Queue<P>, tank: &mut Tank, powered: &mut Powered, signal: bool, world: &mut W) -> bool {
    if let Some(fluid) = tank.fluid.clone().filter(|_| tank.amount > 0) {
        let insert = world.would_take(&fluid, tank.amount);
        if insert > 0
            && let Some((fluid, amount)) = tank.drain(insert.min(BUCKET), false)
        {
            world.fill(&fluid, amount);
        }
    }
    let spot = q.tick(true, powered.has(FLUID_COST), signal, || world.find());
    let Some(at) = spot else { return false };
    let Some(fluid) = world.source(&at) else { return false };
    // `isBlockValidForTank`, then room for a bucket.
    if tank.amount > 0 && tank.fluid.as_deref() != Some(fluid.as_str()) {
        return false;
    }
    if tank.fill(&fluid, BUCKET, FLUID_CAPACITY, true) < BUCKET || !world.take(&at) {
        return false;
    }
    tank.fill(&fluid, BUCKET, FLUID_CAPACITY, false);
    powered.take(FLUID_COST);
    true
}

/// One server tick of the Fluid Placer T2 (`FluidPlacerT1BE.tickServer` with `canPlace` and `placeFluid` of the T2): the
/// slot's container fills the tank, then a place of the area gets a bucket of its fluid, 500 FE each. True when it placed.
pub fn placer_tick<P: Clone>(q: &mut Queue<P>, tank: &mut Tank, powered: &mut Powered, signal: bool, slot: &mut dyn Bucket, world: &mut dyn Pools<P>) -> bool {
    fill_tank_from(tank, slot, FLUID_CAPACITY, |_| true);
    let fluid = tank.fluid.clone().filter(|_| tank.amount >= BUCKET);
    let spot = q.tick(fluid.is_some(), powered.has(FLUID_COST), signal, || world.find());
    let (Some(at), Some(fluid)) = (spot, fluid) else { return false };
    if !world.place(&at, &fluid) {
        return false;
    }
    tank.drain(BUCKET, false);
    powered.take(FLUID_COST);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_area_is_paid_for_by_the_floors_of_its_box() {
        assert_eq!(area_volume([10.0, 64.0, 10.0], [11.0, 65.0, 11.0]), 1);
        assert_eq!(area_volume([9.0, 63.0, 9.0], [12.0, 66.0, 12.0]), 27);
        // A half radius: 2 blocks wide.
        assert_eq!(area_volume([10.0, 64.0, 10.0], [12.0, 65.0, 11.0]), 2);
        let mut p = Powered { energy: 30 };
        assert!(!pay_all(&mut p, 54), "short: it still takes what there is");
        assert_eq!(p.energy, 0);
        p.energy = 100;
        assert!(pay_all(&mut p, 54) && p.energy == 46);
    }

    #[test]
    fn the_filter_lists_one_slot_for_each_item_or_none() {
        let slots = [None, Some('a'), Some('b'), Some('a')];
        assert_eq!(populate_filtered(&['b', 'a'], &slots, |f, s| f == s), vec![2, 1]);
        assert_eq!(populate_filtered(&['a', 'a'], &slots, |f, s| f == s), vec![1, 1], "alike: the slot twice");
        assert!(populate_filtered(&['b', 'c'], &slots, |f, s| f == s).is_empty(), "one missing clears all");
        assert_eq!(populate_plain(&[false, false, true, true]), vec![2]);
        assert!(populate_plain(&[false; 9]).is_empty());
    }

    #[test]
    fn the_dropper_t2_drops_one_listed_slot_each_operation() {
        let mut d = DropperT2::default();
        let mut drops = Vec::new();
        for t in 1..=65 {
            if let Some(s) = d.tick(false, true, || vec![3, 5], |_| true) {
                drops.push((t, s));
            }
        }
        assert_eq!(drops, vec![(20, 3), (40, 5), (60, 3)], "listed at 20, a slot each run, listed again once the list is empty");
        // No power: the list is dropped.
        let mut d = DropperT2 { list: vec![1], ..DropperT2::default() };
        assert_eq!(d.tick(false, false, || vec![2], |_| true), None);
        assert!(d.list.is_empty());
        // The slot emptied meanwhile: the list goes.
        let mut d = DropperT2 { list: vec![1, 2], ..DropperT2::default() };
        d.clock.operation_ticks = 1;
        assert_eq!(d.tick(false, true, Vec::new, |_| false), None);
        assert!(d.list.is_empty());
    }

    #[test]
    fn the_dropper_t2_on_a_pulse_drops_at_once() {
        let mut d = DropperT2::default();
        d.clock.redstone.mode = Mode::Pulse;
        assert_eq!(d.tick(true, true, || vec![0], |_| true), Some(0));
        assert_eq!(d.tick(true, true, || vec![0], |_| true), None, "a held signal is no new pulse");
    }

    #[derive(Default)]
    struct Field {
        sources: Vec<(i32, &'static str)>,
        placed: Vec<(i32, String)>,
        taken: Vec<i32>,
        places: Vec<i32>,
        bucket: Option<(String, i32)>,
        filled: Vec<(String, i32)>,
    }

    impl Pools<i32> for Field {
        fn find(&mut self) -> Vec<i32> {
            self.places.clone()
        }
        fn source(&mut self, at: &i32) -> Option<String> {
            self.sources.iter().find(|(p, _)| p == at).map(|(_, f)| (*f).to_owned())
        }
        fn take(&mut self, at: &i32) -> bool {
            self.taken.push(*at);
            self.sources.retain(|(p, _)| p != at);
            true
        }
        fn place(&mut self, at: &i32, fluid: &str) -> bool {
            self.placed.push((*at, fluid.to_owned()));
            true
        }
    }

    impl Container for Field {
        fn would_take(&mut self, _: &str, amount: i32) -> i32 {
            if self.bucket.is_some() && amount >= BUCKET { BUCKET } else { 0 }
        }
        fn fill(&mut self, fluid: &str, amount: i32) {
            self.filled.push((fluid.to_owned(), amount));
            self.bucket = None;
        }
    }

    #[test]
    fn the_fluid_collector_t2_takes_one_source_a_run_for_500_fe() {
        let mut f = Field { sources: vec![(1, "minecraft:water"), (2, "minecraft:water"), (3, "minecraft:lava")], places: vec![1, 2, 3], ..Field::default() };
        let (mut q, mut tank, mut p) = (Queue::<i32>::default(), Tank::default(), Powered { energy: 1200 });
        let mut got = Vec::new();
        for t in 1..=80 {
            if collector_tick(&mut q, &mut tank, &mut p, false, &mut f) {
                got.push(t);
            }
        }
        // Found at 20 (1 taken), 40 (2 taken), 60: lava is not for a tank of water, and the energy is 200 short of a third.
        assert_eq!(got, vec![20, 40]);
        assert_eq!((f.taken.clone(), tank.amount, p.energy), (vec![1, 2], 2000, 200));
        // The container takes a bucket out of the tank.
        f.bucket = Some(("x".into(), 1));
        collector_tick(&mut q, &mut tank, &mut p, false, &mut f);
        assert_eq!((f.filled.len(), tank.amount), (1, 1000));
    }

    struct Pail(Option<(&'static str, i32)>);

    impl Bucket for Pail {
        fn peek(&mut self) -> Option<(String, i32)> {
            self.0.map(|(f, a)| (f.to_owned(), a))
        }
        fn drain(&mut self, _: i32) {
            self.0 = None;
        }
    }

    #[test]
    fn the_fluid_placer_t2_places_a_bucket_a_run_for_500_fe() {
        let mut f = Field { places: vec![7, 8], ..Field::default() };
        let (mut q, mut tank, mut p) = (Queue::<i32>::default(), Tank::default(), Powered { energy: 800 });
        let mut pail = Pail(Some(("minecraft:water", 1000)));
        let mut done = 0;
        for _ in 1..=80 {
            if placer_tick(&mut q, &mut tank, &mut p, false, &mut pail, &mut f) {
                done += 1;
            }
        }
        // One bucket from the pail: one placement, then the tank is empty.
        assert_eq!((done, f.placed.clone(), tank.amount, p.energy), (1, vec![(7, "minecraft:water".to_owned())], 0, 300));
        // A tank with 2 buckets but 300 FE: nothing.
        tank = Tank { fluid: Some("minecraft:water".into()), amount: 2000 };
        for _ in 1..=60 {
            placer_tick(&mut q, &mut tank, &mut p, false, &mut Pail(None), &mut f);
        }
        assert_eq!(f.placed.len(), 1, "too little energy");
        assert!(in_both(([0.0; 3], [2.0; 3]), ([1.0; 3], [3.0; 3]), [1.5; 3]) && !in_both(([0.0; 3], [2.0; 3]), ([1.0; 3], [3.0; 3]), [0.5; 3]));
    }
}
