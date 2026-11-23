//! The Block Placer T1 (`BlockPlacerT1BE`) and the Fluid Collector T1 (`FluidCollectorT1BE`), on `BaseMachineBE`'s
//! operation clock and the redstone control (`RedstoneControlledBE`). Every `tickSpeed` ticks (20), or on each pulse in
//! pulse mode, the placer places the block item of its slot in front of it where the block there can be replaced, and
//! the collector takes the source block of a fluid in front of it into its 8,000 mB tank; each tick before that the
//! collector fills the fluid container in its slot from the tank, a bucket at a time.
use crate::{
    paradox::Area,
    redstone::{Mode, Redstone},
};

/// The most a machine's tick speed goes to (`TICKSPEEDBUTTON`'s maximum).
pub const MAX_TICK_SPEED: i32 = 1200;

/// `BaseMachineBE`'s operation clock with the machine's redstone control. `redstone` and `tick_speed` are saved
/// (`redstoneMode`, `pulsed`, `receivingRedstone`, `tickspeed`); `operation_ticks` is Java's transient field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub redstone: Redstone,
    pub tick_speed: i32,
    pub operation_ticks: i32,
}

impl Default for Clock {
    fn default() -> Self {
        Self { redstone: Redstone::default(), tick_speed: 20, operation_ticks: -1 }
    }
}

impl Clock {
    /// `BaseMachineBE.tickServer`: `handleTicks`, then `evaluateRedstone` with `hasNeighborSignal`.
    pub fn tick(&mut self, signal: bool) {
        if self.operation_ticks <= 0 {
            self.operation_ticks = self.tick_speed;
        }
        self.operation_ticks -= 1;
        self.redstone.evaluate(signal);
    }

    /// `canRun`: on the clock's tick, or any tick in pulse mode.
    pub fn can_run(&self) -> bool {
        self.operation_ticks == 0 || self.redstone.mode == Mode::Pulse
    }

    /// `setTickSpeed` from the screen's number button, within `minimum_machine_tick_speed` and 1200.
    pub fn set_tick_speed(&mut self, ticks: i32, min: i32) {
        self.tick_speed = ticks.clamp(min.max(1), MAX_TICK_SPEED);
        if self.operation_ticks > self.tick_speed {
            self.operation_ticks = self.tick_speed;
        }
    }
}

/// The block in front of a placer, as its tick sees it.
pub trait Spot {
    /// `isBlockPosValid`: the block there can be replaced (`canBeReplaced()`).
    fn valid(&mut self) -> bool;
    /// `placeBlock`: the slot's item used on it as the machine's fake player would (`ItemStack.useOn`).
    fn place(&mut self);
}

/// The Block Placer T1. `direction` (`getDirectionValue`, an index of `Direction.values()`, 0 down by default) is the
/// face its fake player clicks, which sets the way a log or a stair faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Placer {
    pub clock: Clock,
    pub direction: i32,
}

impl Placer {
    /// One server tick (`tickServer` then `doBlockPlace`). `block_item`: the slot holds a block item
    /// (`isStackValid`). True when it placed (tried to place) this tick.
    pub fn tick(&mut self, block_item: bool, signal: bool, front: &mut dyn Spot) -> bool {
        self.clock.tick(signal);
        if !block_item {
            self.clock.redstone.pulsed = false;
            return false;
        }
        // `findSpotsToPlace` then the placement of the one spot, in the same tick: the tracker is empty at every
        // tick's start, so `clearTrackerIfNeeded` never clears anything for the T1.
        if self.clock.redstone.is_active() && self.clock.can_run() && front.valid() {
            front.place();
            return true;
        }
        false
    }
}

/// A bucket's worth (`FluidType.BUCKET_VOLUME`).
pub const BUCKET: i32 = 1000;
/// The Fluid Collector T1's tank (`getMaxMB`).
pub const COLLECTOR_CAPACITY: i32 = 8000;

/// A machine's one tank (`JustDireFluidTank`): one fluid, by its registry name, and how much of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tank {
    pub fluid: Option<String>,
    pub amount: i32,
}

impl Tank {
    /// `fill`: up to `amount` of `fluid` while the tank is empty or holds that fluid; what it took (with
    /// `simulate`, would take).
    pub fn fill(&mut self, fluid: &str, amount: i32, capacity: i32, simulate: bool) -> i32 {
        if amount <= 0 || (self.amount > 0 && self.fluid.as_deref() != Some(fluid)) {
            return 0;
        }
        let took = amount.min(capacity - self.amount).max(0);
        if took > 0 && !simulate {
            self.fluid = Some(fluid.to_owned());
            self.amount += took;
        }
        took
    }

    /// `drain`: up to `max` of what it holds; the fluid and how much left it (with `simulate`, would leave).
    pub fn drain(&mut self, max: i32, simulate: bool) -> Option<(String, i32)> {
        let took = max.min(self.amount);
        if took <= 0 {
            return None;
        }
        let fluid = self.fluid.clone()?;
        if !simulate {
            self.amount -= took;
            if self.amount == 0 {
                self.fluid = None;
            }
        }
        Some((fluid, took))
    }
}

/// The block in front of a collector.
pub trait Source {
    /// `isBlockPosValid`: the fluid of the `LiquidBlock` there when it is a source (`LEVEL` 0).
    fn source(&mut self) -> Option<String>;
    /// `level.setBlock(pos, AIR, 3)` (with the bucket's fill sound); whether the block changed.
    fn take(&mut self) -> bool;
}

/// The fluid container in a collector's slot (`IFluidHandlerItem`).
pub trait Container {
    /// `fill` simulated: how much of `amount` of `fluid` it would take; 0 for no container.
    fn would_take(&mut self, fluid: &str, amount: i32) -> i32;
    /// `fill` executed (a bucket becomes the full bucket in the slot).
    fn fill(&mut self, fluid: &str, amount: i32);
}

/// What is around a collector: its slot's container and the block in front.
pub trait Around: Container + Source {}

impl<T: Container + Source> Around for T {}

/// The Fluid Collector T1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collector {
    pub clock: Clock,
    pub tank: Tank,
}

impl Collector {
    /// One server tick (`tickServer`, `handleItemStack`, `doFluidCollect`). True when it took a source block.
    pub fn tick(&mut self, signal: bool, around: &mut dyn Around) -> bool {
        self.clock.tick(signal);
        // `handleItemStack`: the slot's container takes what it can of the tank, a bucket at most.
        if let Some(fluid) = self.tank.fluid.clone().filter(|_| self.tank.amount > 0) {
            let insert = around.would_take(&fluid, self.tank.amount);
            if insert > 0
                && let Some((fluid, amount)) = self.tank.drain(insert.min(BUCKET), false)
            {
                around.fill(&fluid, amount);
            }
        }
        // `doFluidCollect`: a source the tank takes a whole bucket of.
        if self.clock.redstone.is_active()
            && self.clock.can_run()
            && let Some(fluid) = around.source()
            && self.tank.fill(&fluid, BUCKET, COLLECTOR_CAPACITY, true) == BUCKET
            && around.take()
        {
            self.tank.fill(&fluid, BUCKET, COLLECTOR_CAPACITY, false);
            return true;
        }
        false
    }
}

/// The Item Collector's filter slots (`FilterBasicHandler(9)`).
pub const FILTER_SLOTS: usize = 9;

/// A machine's filter settings (`FilterData`, saved as `allowlist`, `compareNBT` and `blockitemfilter`; the
/// collector's `blockItemFilter` stays -1, so it has no block/item button).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub allowlist: bool,
    pub compare_nbt: bool,
}

impl Filter {
    /// `isStackValidFilter`: a stack one of the filter's items matches (by item, and by its components when
    /// `compareNBT`) is what the list says, allowed on an allowlist and refused on a denylist; any other the opposite.
    /// An empty denylist (the default) lets everything through.
    pub fn accepts(&self, matched: bool) -> bool {
        if matched { self.allowlist } else { !self.allowlist }
    }
}

/// The Item Collector (`ItemCollectorBE`, with `FilterableBE` and `AreaAffectingBE`): on its operation clock it
/// takes the item entities of its area (by default the one block behind it, `AreaAffectingData(facing.getOpposite())`)
/// into the inventory it faces, stacking them as `ItemHandlerHelper.insertItemStacked` does; what doesn't fit stays
/// on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemCollector {
    pub clock: Clock,
    pub area: Area,
    pub filter: Filter,
    /// `respectPickupDelay`: leave items that can't be picked up yet (off by default).
    pub respect_pickup_delay: bool,
    /// `showParticles` (on by default).
    pub show_particles: bool,
}

impl ItemCollector {
    /// A new collector facing `facing` (a side's normal): its area is the block behind it.
    pub fn new(facing: [i32; 3]) -> Self {
        Self {
            clock: Clock::default(),
            area: Area::facing(facing.map(|v| -v)),
            filter: Filter::default(),
            respect_pickup_delay: false,
            show_particles: true,
        }
    }

    /// One server tick (`tickServer` then `findItemsAndStore`'s first test): whether it collects this tick.
    pub fn tick(&mut self, signal: bool) -> bool {
        self.clock.tick(signal);
        self.clock.redstone.is_active() && self.clock.can_run()
    }

    /// Whether an item entity is collected: not one still waiting to be picked up when the collector respects that,
    /// and one its filter lets through (`matched`: one of the filter's items matches it).
    pub fn takes(&self, pickup_delay: bool, matched: bool) -> bool {
        !(self.respect_pickup_delay && pickup_delay) && self.filter.accepts(matched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Air {
        free: bool,
        placed: Vec<u32>,
        tick: u32,
    }

    impl Spot for Air {
        fn valid(&mut self) -> bool {
            self.free
        }
        fn place(&mut self) {
            self.placed.push(self.tick);
            self.free = false;
        }
    }

    #[test]
    fn the_placer_places_on_the_clocks_tick_where_the_block_can_be_replaced() {
        let mut p = Placer::default();
        let mut front = Air { free: true, placed: Vec::new(), tick: 0 };
        for tick in 1..=45 {
            front.tick = tick;
            if tick == 25 {
                front.free = true;
            }
            p.tick(true, false, &mut front);
        }
        // The clock's first run comes on the 20th tick; the spot freed at 25 is filled on the 40th.
        assert_eq!(front.placed, vec![20, 40]);
    }

    #[test]
    fn the_placer_waits_for_a_block_item_and_its_redstone() {
        let mut p = Placer::default();
        let mut front = Air { free: true, placed: Vec::new(), tick: 0 };
        for _ in 0..40 {
            p.tick(false, false, &mut front);
        }
        assert!(front.placed.is_empty(), "no block item, nothing placed");
        p.clock.redstone.mode = Mode::High;
        for _ in 0..40 {
            p.tick(true, false, &mut front);
        }
        assert!(front.placed.is_empty(), "high with no signal");
        // Pulse: once per rising edge, on any tick; an empty slot drops a pending pulse.
        p.clock.redstone.mode = Mode::Pulse;
        p.tick(true, true, &mut front);
        assert_eq!(front.placed.len(), 1);
        front.free = true;
        p.tick(true, true, &mut front);
        assert_eq!(front.placed.len(), 1, "a held signal is no new pulse");
        p.tick(true, false, &mut front);
        p.clock.redstone.evaluate(true);
        p.tick(false, true, &mut front);
        assert!(!p.clock.redstone.pulsed, "an empty slot drops the pulse");
    }

    #[test]
    fn the_tick_speed_stays_within_its_range_and_pulls_the_clock_in() {
        let mut c = Clock { operation_ticks: 15, ..Clock::default() };
        c.set_tick_speed(5, 1);
        assert_eq!((c.tick_speed, c.operation_ticks), (5, 5));
        c.set_tick_speed(0, 1);
        assert_eq!(c.tick_speed, 1);
        c.set_tick_speed(5000, 1);
        assert_eq!(c.tick_speed, MAX_TICK_SPEED);
        c.set_tick_speed(3, 10);
        assert_eq!(c.tick_speed, 10, "the config's minimum");
    }

    #[test]
    fn a_tank_holds_one_fluid_up_to_its_capacity() {
        let mut t = Tank::default();
        assert_eq!(t.fill("minecraft:water", 1000, 8000, true), 1000);
        assert_eq!(t, Tank::default(), "a simulation changes nothing");
        assert_eq!(t.fill("minecraft:water", 7500, 8000, false), 7500);
        assert_eq!(t.fill("minecraft:water", 1000, 8000, false), 500);
        assert_eq!(t.fill("minecraft:lava", 1000, 8000, false), 0, "another fluid");
        assert_eq!(t.drain(3000, false), Some(("minecraft:water".into(), 3000)));
        assert_eq!(t.drain(9000, false), Some(("minecraft:water".into(), 5000)));
        assert_eq!(t, Tank::default());
        assert_eq!(t.drain(1000, false), None);
        assert_eq!(t.fill("minecraft:lava", 1000, 8000, false), 1000, "empty, it takes any fluid");
    }

    struct Pool {
        fluid: Option<&'static str>,
        taken: u32,
    }

    impl Source for Pool {
        fn source(&mut self) -> Option<String> {
            self.fluid.map(str::to_owned)
        }
        fn take(&mut self) -> bool {
            self.taken += 1;
            true
        }
    }

    /// An empty bucket (taking a whole bucket of anything) or nothing.
    struct Slot {
        bucket: bool,
        filled: Option<(String, i32)>,
    }

    impl Container for Slot {
        fn would_take(&mut self, _: &str, amount: i32) -> i32 {
            if self.bucket && self.filled.is_none() && amount >= BUCKET { BUCKET } else { 0 }
        }
        fn fill(&mut self, fluid: &str, amount: i32) {
            self.filled = Some((fluid.to_owned(), amount));
        }
    }

    /// A pool in front and a slot.
    struct Bench {
        pool: Pool,
        slot: Slot,
    }

    impl Source for Bench {
        fn source(&mut self) -> Option<String> {
            self.pool.source()
        }
        fn take(&mut self) -> bool {
            self.pool.take()
        }
    }

    impl Container for Bench {
        fn would_take(&mut self, fluid: &str, amount: i32) -> i32 {
            self.slot.would_take(fluid, amount)
        }
        fn fill(&mut self, fluid: &str, amount: i32) {
            self.slot.fill(fluid, amount);
        }
    }

    fn bench(fluid: &'static str) -> Bench {
        Bench { pool: Pool { fluid: Some(fluid), taken: 0 }, slot: Slot { bucket: false, filled: None } }
    }

    #[test]
    fn the_collector_takes_a_source_each_operation_until_its_tank_is_full() {
        let mut c = Collector::default();
        let mut b = bench("minecraft:water");
        for _ in 0..(20 * 9) {
            c.tick(false, &mut b);
        }
        assert_eq!(b.pool.taken, 8, "eight buckets fill the tank, the ninth finds no room");
        assert_eq!(c.tank, Tank { fluid: Some("minecraft:water".into()), amount: COLLECTOR_CAPACITY });
        // A bucket in the slot takes one bucket out on the next tick.
        b.slot.bucket = true;
        c.tick(false, &mut b);
        assert_eq!(b.slot.filled, Some(("minecraft:water".into(), BUCKET)));
        assert_eq!(c.tank.amount, COLLECTOR_CAPACITY - BUCKET);
    }

    #[test]
    fn the_collector_leaves_another_fluid_and_follows_its_redstone() {
        let mut c = Collector { tank: Tank { fluid: Some("minecraft:water".into()), amount: 1000 }, ..Collector::default() };
        let mut lava = bench("minecraft:lava");
        for _ in 0..40 {
            c.tick(false, &mut lava);
        }
        assert_eq!(lava.pool.taken, 0, "lava is no use to a tank of water");
        let mut water = bench("minecraft:water");
        c.clock.redstone.mode = Mode::Low;
        for _ in 0..40 {
            c.tick(true, &mut water);
        }
        assert_eq!(water.pool.taken, 0, "low with a signal");
        for _ in 0..20 {
            c.tick(false, &mut water);
        }
        assert_eq!(water.pool.taken, 1);
        assert_eq!(c.tank.amount, 2000);
    }

#[test]
    fn the_item_collector_gathers_behind_it_on_its_clock() {
        // Facing north into a chest: the area is the block to the south.
        let mut c = ItemCollector::new([0, 0, -1]);
        assert_eq!(c.area.bounds([10, 64, 10]), ([10.0, 64.0, 11.0], [11.0, 65.0, 12.0]));
        let runs: Vec<u32> = (1..=45).filter(|_| c.tick(false)).collect();
        assert_eq!(runs, vec![20, 40]);
        c.clock.redstone.mode = Mode::High;
        assert!(!(0..40).any(|_| c.tick(false)), "high with no signal");
    }

    #[test]
    fn the_item_collectors_filter_and_pickup_delay() {
        let mut c = ItemCollector::new([0, 0, -1]);
        assert!(c.takes(true, false) && c.takes(false, false), "nothing matches an empty denylist: it takes everything");
        c.filter.allowlist = true;
        assert!(c.takes(false, true) && !c.takes(false, false), "an allowlist takes only its items");
        c.filter.allowlist = false;
        assert!(!c.takes(false, true), "a denylist leaves its items");
        c.respect_pickup_delay = true;
        assert!(!c.takes(true, false) && c.takes(false, false));
    }
}
