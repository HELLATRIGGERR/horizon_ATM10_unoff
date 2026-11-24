//! The Dropper T1 (`DropperT1BE`), the Fluid Placer T1 (`FluidPlacerT1BE`), the Sensor T1 (`SensorT1BE`) and the
//! Fluid Generator T1 (`GeneratorFluidT1BE`), on `BaseMachineBE`'s operation clock and the redstone control.
use crate::{
    SIDES,
    config::FluidGeneratorSettings,
    machines::{BUCKET, Clock, Tank},
    redstone::Redstone,
};

// ---- The Dropper T1 --------------------------------------------------------------------------------------------

/// The most it drops at once (`DropperT1Screen`'s number button).
pub const MAX_DROP: i32 = 64;
/// The longest pickup delay its button sets.
pub const MAX_DELAY: i32 = 1200;

/// The Dropper T1: on each operation (or pulse) it drops `count` of its slot's item in front of it, thrown the way its
/// `direction` (a `Direction.values()` index, 0 down) points, with a pickup delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dropper {
    pub clock: Clock,
    pub direction: i32,
    pub count: i32,
    pub delay: i32,
}

impl Default for Dropper {
    fn default() -> Self {
        Self { clock: Clock::default(), direction: 0, count: 1, delay: 0 }
    }
}

impl Dropper {
    /// One server tick (`tickServer`, `doDrop`): true when it drops this tick. The list of slots to drop is always
    /// empty at a tick's start (one slot, and it is emptied the tick it is filled), so `isActiveRedstone` runs first,
    /// and a pulse found forces the clock to 0.
    pub fn tick(&mut self, has_item: bool, signal: bool) -> bool {
        self.clock.tick(signal);
        let pulse_pending = self.clock.redstone.mode == crate::redstone::Mode::Pulse && self.clock.redstone.pulsed;
        let active = self.clock.redstone.is_active();
        if pulse_pending {
            self.clock.operation_ticks = 0;
        }
        active && self.clock.can_run() && has_item && self.clock.operation_ticks == 0
    }

    pub fn set_count(&mut self, count: i32) {
        self.count = count.clamp(1, MAX_DROP);
    }

    pub fn set_delay(&mut self, delay: i32) {
        self.delay = delay.clamp(0, MAX_DELAY);
    }
}

// ---- The Fluid Placer T1 ---------------------------------------------------------------------------------------

/// The Fluid Placer T1's tank (`getMaxMB`).
pub const PLACER_CAPACITY: i32 = 8000;

/// The fluid container in a machine's slot (`IFluidHandlerItem`).
pub trait Bucket {
    /// `drain(1000, SIMULATE)`: the fluid and amount it would give.
    fn peek(&mut self) -> Option<(String, i32)>;
    /// `drain(amount, EXECUTE)` (a bucket becomes the empty one).
    fn drain(&mut self, amount: i32);
}

/// The block in front of a fluid placer.
pub trait FluidSpot {
    /// `isBlockPosValid`: replaceable and not already a source.
    fn valid(&mut self) -> bool;
    /// `FluidUtil.tryPlaceFluid` of a bucket of `fluid`: whether it was placed (or evaporated).
    fn place(&mut self, fluid: &str) -> bool;
}

/// The Fluid Placer T1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FluidPlacer {
    pub clock: Clock,
    pub tank: Tank,
}

impl FluidPlacer {
    /// `handleItemStack`: the slot's container gives the tank up to a bucket of what it holds, when it is the tank's fluid.
    fn fill_from(&mut self, slot: &mut dyn Bucket, capacity: i32, accepts: impl Fn(&str) -> bool) {
        if self.tank.amount >= capacity {
            return;
        }
        let Some((fluid, amount)) = slot.peek() else { return };
        if amount <= 0 || !accepts(&fluid) || (self.tank.amount > 0 && self.tank.fluid.as_deref() != Some(&fluid)) {
            return;
        }
        let insert = self.tank.fill(&fluid, amount, capacity, true);
        if insert > 0 {
            slot.drain(insert);
            self.tank.fill(&fluid, insert, capacity, false);
        }
    }

    /// One server tick (`tickServer`, `handleItemStack`, `doFluidPlace`). True when it placed a bucket.
    pub fn tick(&mut self, signal: bool, slot: &mut dyn Bucket, front: &mut dyn FluidSpot) -> bool {
        self.clock.tick(signal);
        self.fill_from(slot, PLACER_CAPACITY, |_| true);
        let Some(fluid) = self.tank.fluid.clone().filter(|_| self.tank.amount >= BUCKET) else {
            self.clock.redstone.pulsed = false;
            return false;
        };
        if self.clock.redstone.is_active() && self.clock.can_run() && front.valid() && front.place(&fluid) {
            self.tank.drain(BUCKET, false);
            return true;
        }
        false
    }
}

// ---- The Sensor T1 ---------------------------------------------------------------------------------------------

/// `SensorT1BE.SENSE_TARGET`, in its ordinal order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Block,
    Air,
    Hostile,
    Passive,
    Adult,
    Child,
    Player,
    Living,
    Item,
}

impl Target {
    pub const ALL: [Self; 9] =
        [Self::Block, Self::Air, Self::Hostile, Self::Passive, Self::Adult, Self::Child, Self::Player, Self::Living, Self::Item];

    pub fn index(self) -> i32 {
        self as i32
    }

    pub fn from_index(i: i32) -> Self {
        Self::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 9)
    }

    /// Whether it senses the block in front (and not entities there).
    pub fn is_block(self) -> bool {
        matches!(self, Self::Block | Self::Air)
    }
}

/// The Sensor T1's settings: what it looks for, whether its signal is strong, and the count test of `checkCount`
/// (the T1's screen leaves them at 0: more than none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sensor {
    pub clock: Clock,
    pub target: Target,
    pub strong: bool,
    pub amount: i32,
    /// 0 greater than, 1 less than, 2 equals.
    pub equality: i32,
}

impl Sensor {
    /// `checkCount`.
    pub fn check_count(&self, found: i32) -> bool {
        match self.equality {
            1 => found < self.amount,
            2 => found == self.amount,
            _ => found > self.amount,
        }
    }

    /// One server tick's clock (`tickServer`); `canRun`: whether it senses now.
    pub fn tick(&mut self) -> bool {
        self.clock.tick(false);
        self.clock.operation_ticks == 0
    }
}

// ---- The Fluid Generator T1 ------------------------------------------------------------------------------------

/// The Fluid Generator T1's tank (`getMaxMB`).
pub const GENERATOR_CAPACITY: i32 = 4000;

/// The refined fuels, by their fluid's name, with the config's FE for each millibucket (`RefinedFuel.fePerMb`); 0 for
/// any other fluid.
pub fn fe_per_mb(s: &FluidGeneratorSettings, fluid: &str) -> i32 {
    match fluid {
        "justdirethings:refined_t2_fluid_source" | "justdirethings:refined_t2_fluid_flowing" => s.tier2,
        "justdirethings:refined_t3_fluid_source" | "justdirethings:refined_t3_fluid_flowing" => s.tier3,
        "justdirethings:refined_t4_fluid_source" | "justdirethings:refined_t4_fluid_flowing" => s.tier4,
        _ => 0,
    }
}

/// The Fluid Generator T1: each tick it burns a millibucket of refined fuel for its FE while the buffer has room for all
/// of it, and hands the buffer to its neighbours.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FluidGenerator {
    pub energy: i32,
    pub tank: Tank,
    pub redstone: Redstone,
}

impl FluidGenerator {
    /// `handleItemStack` (only refined fuel is valid), then `doGenerate`. `slot`: the container in its slot.
    pub fn tick(&mut self, s: &FluidGeneratorSettings, signal: bool, slot: &mut dyn Bucket) {
        self.redstone.evaluate(signal);
        let mut tank = std::mem::take(&mut self.tank);
        let mut filler = FluidPlacer { clock: Clock::default(), tank: std::mem::take(&mut tank) };
        filler.fill_from(slot, GENERATOR_CAPACITY, |f| fe_per_mb(s, f) > 0);
        self.tank = filler.tank;
        if !self.redstone.is_active() {
            return;
        }
        let Some(fluid) = self.tank.fluid.clone().filter(|_| self.tank.amount > 0) else { return };
        let fe = fe_per_mb(s, &fluid);
        if fe == 0 || i64::from(self.energy) + i64::from(fe) > i64::from(s.max_fe) {
            return;
        }
        if self.tank.drain(1, false).is_some() {
            self.energy += fe;
        }
    }

    /// `extractEnergy`.
    pub fn extract(&mut self, s: &FluidGeneratorSettings, amount: i64, simulate: bool) -> i64 {
        let given = i64::from(self.energy).min(i64::from(s.max_fe).min(amount)).max(0);
        if !simulate {
            self.energy -= given as i32;
        }
        given
    }

    /// `providePowerAdjacent`: each side is offered ten times the config's FE a tick, as much as it would take is drawn.
    pub fn push(&mut self, s: &FluidGeneratorSettings, mut offer: impl FnMut(usize, i64, bool) -> i64) {
        if self.energy <= 0 {
            return;
        }
        for side in 0..SIDES {
            let fits = offer(side, i64::from(s.fe_per_tick.saturating_mul(10)), true);
            if fits <= 0 {
                continue;
            }
            let drawn = self.extract(s, fits, false);
            offer(side, drawn, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone::Mode;

    #[test]
    fn the_dropper_drops_on_the_clock_and_on_a_pulse() {
        let mut d = Dropper::default();
        let drops: Vec<u32> = (1..=45).filter(|_| d.tick(true, false)).collect();
        assert_eq!(drops, vec![20, 40]);
        let mut d = Dropper::default();
        assert!(!(0..40).any(|_| d.tick(false, false)), "nothing to drop");
        d.clock.redstone.mode = Mode::High;
        assert!(!(0..40).any(|_| d.tick(true, false)), "high with no signal");
        d.clock.redstone.mode = Mode::Pulse;
        d.clock.operation_ticks = 7;
        assert!(d.tick(true, true), "a pulse drops at once, whatever the clock");
        assert!(!d.tick(true, true), "a held signal is no new pulse");
        d.set_count(100);
        d.set_delay(-3);
        assert_eq!((d.count, d.delay), (64, 0));
    }

    struct Pool {
        free: bool,
        placed: Vec<String>,
    }

    impl FluidSpot for Pool {
        fn valid(&mut self) -> bool {
            self.free
        }
        fn place(&mut self, fluid: &str) -> bool {
            self.placed.push(fluid.to_owned());
            true
        }
    }

    struct Pail {
        fluid: Option<(&'static str, i32)>,
    }

    impl Bucket for Pail {
        fn peek(&mut self) -> Option<(String, i32)> {
            self.fluid.map(|(f, a)| (f.to_owned(), a))
        }
        fn drain(&mut self, _: i32) {
            self.fluid = None;
        }
    }

    #[test]
    fn the_fluid_placer_fills_from_its_bucket_and_places_a_bucket_each_operation() {
        let mut p = FluidPlacer::default();
        let mut pail = Pail { fluid: Some(("minecraft:water", 1000)) };
        let mut front = Pool { free: true, placed: Vec::new() };
        p.tick(false, &mut pail, &mut front);
        assert_eq!(p.tank.amount, 1000);
        assert!(pail.fluid.is_none());
        for _ in 0..19 {
            p.tick(false, &mut pail, &mut front);
        }
        assert_eq!(front.placed, vec!["minecraft:water".to_owned()]);
        assert_eq!(p.tank.amount, 0);
        assert_eq!(p.tank.fluid, None);
    }

    #[test]
    fn the_fluid_placer_leaves_a_taken_spot_and_other_fluids_out() {
        let mut p = FluidPlacer { tank: Tank { fluid: Some("minecraft:water".into()), amount: 2000 }, ..FluidPlacer::default() };
        let mut pail = Pail { fluid: Some(("minecraft:lava", 1000)) };
        let mut front = Pool { free: false, placed: Vec::new() };
        for _ in 0..40 {
            p.tick(false, &mut pail, &mut front);
        }
        assert!(front.placed.is_empty(), "the spot is taken");
        assert!(pail.fluid.is_some(), "lava does not mix with the water in the tank");
        front.free = true;
        for _ in 0..20 {
            p.tick(false, &mut pail, &mut front);
        }
        assert_eq!(front.placed.len(), 1);
        assert_eq!(p.tank.amount, 1000);
    }

    #[test]
    fn the_sensors_count_test() {
        let mut s = Sensor::default();
        assert!(s.check_count(1) && !s.check_count(0), "more than none");
        s.equality = 1;
        s.amount = 3;
        assert!(s.check_count(2) && !s.check_count(3));
        s.equality = 2;
        assert!(s.check_count(3) && !s.check_count(2));
        let runs: Vec<u32> = (1..=45).filter(|_| s.tick()).collect();
        assert_eq!(runs, vec![20, 40]);
        assert_eq!(Target::from_index(8), Target::Item);
        assert_eq!(Target::Item.next(), Target::Block);
        assert!(Target::Air.is_block() && !Target::Hostile.is_block());
    }

    #[test]
    fn the_fluid_generator_burns_refined_fuel_while_its_buffer_has_room() {
        let s = FluidGeneratorSettings::default();
        let mut g = FluidGenerator { tank: Tank { fluid: Some("justdirethings:refined_t2_fluid_source".into()), amount: 3 }, ..FluidGenerator::default() };
        let mut none = Pail { fluid: None };
        g.tick(&s, false, &mut none);
        g.tick(&s, false, &mut none);
        assert_eq!((g.energy, g.tank.amount), (900, 1));
        g.energy = s.max_fe - 449;
        g.tick(&s, false, &mut none);
        assert_eq!((g.energy, g.tank.amount), (s.max_fe - 449, 1), "no room for a whole millibucket's FE");
        g.redstone.mode = Mode::High;
        g.energy = 0;
        g.tick(&s, false, &mut none);
        assert_eq!(g.energy, 0, "high with no signal");
        let mut water = FluidGenerator::default();
        water.tick(&s, false, &mut Pail { fluid: Some(("minecraft:water", 1000)) });
        assert_eq!(water.tank.amount, 0, "only refined fuel goes in");
        let mut fuel = FluidGenerator::default();
        fuel.tick(&s, false, &mut Pail { fluid: Some(("justdirethings:refined_t4_fluid_source", 1000)) });
        assert_eq!(fuel.energy, 4000, "a bucket fills the tank and one millibucket burns");
        assert_eq!(fuel.tank.amount, 999);
    }

    #[test]
    fn the_fluid_generator_hands_its_buffer_to_its_neighbours() {
        let s = FluidGeneratorSettings::default();
        let mut g = FluidGenerator { energy: 1_000_000, ..FluidGenerator::default() };
        let mut taken = [0i64; SIDES];
        g.push(&s, |side, amount, simulate| {
            if side != 3 {
                return 0;
            }
            if !simulate {
                taken[side] += amount;
            }
            amount
        });
        assert_eq!(taken[3], 50_000);
        assert_eq!(g.energy, 950_000);
    }
}
