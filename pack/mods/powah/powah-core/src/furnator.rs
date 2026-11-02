//! A Furnator's burn and push, as Powah 6.2.10 plays on a Java server (the
//! traces are summed up in the bundle's NOTES.md).
//!
//! Each tick, in this order:
//! 1. with no fuel burning, one item of the fuel slot starts to, full or not;
//! 2. the burning fuel makes the tier's generation, as much as is left of it and
//!    as fits; it is `burning` only while it really made something;
//! 3. each of the six sides, down, up, north, south, west, east, is offered up to
//!    the tier's transfer rate of what is stored, so the rate is per side.
//!
//! Fuel is held as the FE it still gives: burn ticks times `energy_per_fuel_tick`.
//! A higher tier burns it faster for the same energy.
use crate::{config::GeneratorSpec, view::FurnatorView};

/// The slot for an item to charge. It takes nothing until items hold energy
/// (Powah's batteries); hoppers can still empty it.
pub const CHARGE_SLOT: usize = 0;
/// The fuel slot.
pub const FUEL_SLOT: usize = 1;
/// The sides a Furnator pushes through, in Java's `Direction` order.
pub const SIDES: usize = 6;
/// The component of the item a player's break drops: a compound with what the
/// block keeps (energy and the fuel burning, under the block's own save keys),
/// as on Java. The block takes it back when the item is placed.
pub const STORED_STATE: &str = "powah:stored_block_entity_state";

/// What a Furnator keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Furnator {
    /// FE stored.
    pub stored: i64,
    /// FE the burning fuel still gives.
    pub carbon: i64,
    /// FE that fuel gives in all.
    pub carbon_max: i64,
    /// It made energy on its last tick.
    pub burning: bool,
}

/// What `burn_ticks` of furnace fuel are worth.
pub fn fuel_value(burn_ticks: i32, energy_per_fuel_tick: i64) -> i64 {
    i64::from(burn_ticks.max(0)).saturating_mul(energy_per_fuel_tick)
}

/// Whether a slot lets a stack in from a click or a hopper; `fuel`: it burns in
/// a furnace.
pub fn slot_accepts(slot: usize, fuel: bool) -> bool {
    slot == FUEL_SLOT && fuel
}

impl Furnator {
    /// Whether the next fuel item can start burning.
    pub fn wants_fuel(&self) -> bool {
        self.carbon <= 0
    }

    /// Starts a fuel item worth `value` FE.
    pub fn start_fuel(&mut self, value: i64) {
        self.carbon = value;
        self.carbon_max = value;
    }

    /// Step 2 of a tick: what the burning fuel made.
    pub fn generate(&mut self, spec: &GeneratorSpec) -> i64 {
        let room = (spec.capacity - self.stored).max(0);
        let made = spec.generation.min(room).min(self.carbon).max(0);
        self.burning = made > 0;
        self.stored += made;
        self.carbon -= made;
        if self.carbon <= 0 {
            self.carbon = 0;
            self.carbon_max = 0;
        }
        made
    }

    /// Step 3 of a tick: offers each side what it may have; `offer(side, amount)`
    /// answers what that side took. What left in all.
    pub fn push(&mut self, spec: &GeneratorSpec, mut offer: impl FnMut(usize, i64) -> i64) -> i64 {
        let mut pushed = 0;
        for side in 0..SIDES {
            let amount = spec.transfer.min(self.stored);
            if amount <= 0 {
                break;
            }
            let taken = offer(side, amount).clamp(0, amount);
            self.stored -= taken;
            pushed += taken;
        }
        pushed
    }

    /// `IEnergyStorage.extractEnergy`: up to the tier's rate a call.
    pub fn extract(&mut self, spec: &GeneratorSpec, max: i64, simulate: bool) -> i64 {
        let given = max.min(spec.transfer).min(self.stored).max(0);
        if !simulate {
            self.stored -= given;
        }
        given
    }

    /// What its screen shows.
    pub fn view(&self, spec: &GeneratorSpec) -> FurnatorView {
        FurnatorView {
            stored: self.stored,
            capacity: spec.capacity,
            fuel_left: self.carbon,
            fuel_total: self.carbon_max,
            generation: spec.generation,
            transfer: spec.transfer,
            burning: self.burning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::PowahConfig, tier::Tier};

    const COAL: i32 = 1600;
    const STICK: i32 = 100;

    fn spec(tier: Tier) -> GeneratorSpec {
        PowahConfig::default().furnators.spec(tier)
    }

    /// One tick with `fuel` in the slot; how much of it is left, and what each
    /// side took. Sides with a sink take all they are offered.
    fn tick(f: &mut Furnator, spec: &GeneratorSpec, fuel: &mut (i32, u32), sinks: [bool; SIDES]) -> [i64; SIDES] {
        if f.wants_fuel() && fuel.0 > 0 && fuel.1 > 0 {
            f.start_fuel(fuel_value(fuel.0, 30));
            fuel.1 -= 1;
        }
        f.generate(spec);
        let mut took = [0; SIDES];
        f.push(spec, |side, amount| {
            let taken = if sinks[side] { amount } else { 0 };
            took[side] = taken;
            taken
        });
        took
    }

    const NO_SINK: [bool; SIDES] = [false; SIDES];

    #[test]
    fn a_coal_is_worth_48000_fe_at_every_tier() {
        assert_eq!(fuel_value(COAL, 30), 48_000);
        assert_eq!(fuel_value(0, 30), 0);
        assert_eq!(fuel_value(-5, 30), 0);
        for tier in Tier::ALL {
            let spec = spec(tier);
            let mut f = Furnator::default();
            let mut fuel = (COAL, 1);
            let mut ticks = 0;
            let mut sink = 0;
            while f.stored + sink < 48_000 {
                let took = tick(&mut f, &spec, &mut fuel, [false, false, true, false, false, false]);
                sink += took[2];
                ticks += 1;
                assert!(ticks < 1000, "{tier:?} never burnt its coal");
            }
            assert_eq!(f.stored + sink, 48_000, "{tier:?}");
            assert_eq!((f.carbon, f.carbon_max), (0, 0), "{tier:?}");
            // 600 ticks at 80 FE/t ... 3 ticks at 20,000 FE/t.
            assert_eq!(ticks, 48_000 / spec.generation + i64::from(48_000 % spec.generation != 0), "{tier:?}");
        }
    }

    #[test]
    fn a_starter_burns_as_the_java_trace_does() {
        // Java trace: 80 FE made and 80 taken off the fuel each tick, the first item going on tick 1.
        let spec = spec(Tier::Starter);
        let mut f = Furnator::default();
        let mut fuel = (COAL, 2);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 80, carbon: 47_920, carbon_max: 48_000, burning: true });
        assert_eq!(fuel.1, 1);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 160, carbon: 47_840, carbon_max: 48_000, burning: true });
    }

    #[test]
    fn the_last_of_a_fuel_makes_what_is_left_and_the_next_starts_a_tick_later() {
        // Java trace with sticks, 100 fuel ticks: 3000 FE in 38 ticks, the last giving 40.
        let spec = spec(Tier::Starter);
        let mut f = Furnator::default();
        let mut fuel = (STICK, 2);
        for _ in 0..37 {
            tick(&mut f, &spec, &mut fuel, NO_SINK);
        }
        assert_eq!(f, Furnator { stored: 2960, carbon: 40, carbon_max: 3000, burning: true });
        assert_eq!(fuel.1, 1);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 3000, carbon: 0, carbon_max: 0, burning: true });
        assert_eq!(fuel.1, 1);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 3080, carbon: 2920, carbon_max: 3000, burning: true });
        assert_eq!(fuel.1, 0);
    }

    #[test]
    fn it_stops_when_full_and_keeps_its_fuel() {
        // Java trace: from 19,900 of 20,000 it makes 80, then 20, then stops.
        let spec = spec(Tier::Starter);
        let mut f = Furnator { stored: 19_900, ..Default::default() };
        let mut fuel = (COAL, 2);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 19_980, carbon: 47_920, carbon_max: 48_000, burning: true });
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 20_000, carbon: 47_900, carbon_max: 48_000, burning: true });
        for _ in 0..3 {
            tick(&mut f, &spec, &mut fuel, NO_SINK);
            assert_eq!(f, Furnator { stored: 20_000, carbon: 47_900, carbon_max: 48_000, burning: false });
        }
        assert_eq!(fuel.1, 1);
    }

    #[test]
    fn a_full_one_with_no_fuel_burning_still_takes_an_item() {
        // Java trace: full, coal in the slot: the coal goes, nothing is made.
        let spec = spec(Tier::Starter);
        let mut f = Furnator { stored: 20_000, ..Default::default() };
        let mut fuel = (COAL, 2);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!(f, Furnator { stored: 20_000, carbon: 48_000, carbon_max: 48_000, burning: false });
        assert_eq!(fuel.1, 1);
    }

    #[test]
    fn generation_goes_out_in_the_same_tick() {
        // Java trace, once the neighbour is there: everything made leaves the same tick.
        let spec = spec(Tier::Starter);
        let mut f = Furnator { stored: 80, carbon: 47_920, carbon_max: 48_000, burning: true };
        let mut fuel = (COAL, 0);
        let took = tick(&mut f, &spec, &mut fuel, [false, false, true, false, false, false]);
        assert_eq!(took[2], 160);
        assert_eq!(f.stored, 0);
    }

    #[test]
    fn the_rate_is_per_side_and_sides_go_in_java_order() {
        // Java trace: 600 FE with six sinks gives down 240, up 240, north 120.
        let spec = spec(Tier::Starter);
        let mut f = Furnator { stored: 600, ..Default::default() };
        let took = tick(&mut f, &spec, &mut (0, 0), [true; SIDES]);
        assert_eq!(took, [240, 240, 120, 0, 0, 0]);
        assert_eq!(f.stored, 0);
        // Full with three sinks: 240 each, 720 in all.
        let mut f = Furnator { stored: 20_000, ..Default::default() };
        let took = tick(&mut f, &spec, &mut (0, 0), [false, false, true, true, false, true]);
        assert_eq!(took, [0, 0, 240, 240, 0, 240]);
        assert_eq!(f.stored, 19_280);
    }

    #[test]
    fn a_side_that_takes_less_keeps_the_rest_home() {
        let spec = spec(Tier::Basic);
        let mut f = Furnator { stored: 1000, ..Default::default() };
        let pushed = f.push(&spec, |side, amount| match side {
            0 => 100,
            1 => amount + 50,
            2 => -5,
            _ => 0,
        });
        assert_eq!(pushed, 100 + 480);
        assert_eq!(f.stored, 1000 - 580);
    }

    #[test]
    fn every_tier_pushes_its_own_rate() {
        for tier in Tier::ALL {
            let spec = spec(tier);
            let mut f = Furnator { stored: spec.capacity, ..Default::default() };
            let pushed = f.push(&spec, |side, amount| if side == 3 { amount } else { 0 });
            assert_eq!(pushed, spec.transfer, "{tier:?}");
        }
        // Java trace, nitro: a coal's 48,000 FE goes 20,000, 20,000, 8,000 and all leaves.
        let spec = spec(Tier::Nitro);
        let mut f = Furnator::default();
        let mut fuel = (COAL, 1);
        tick(&mut f, &spec, &mut fuel, NO_SINK);
        assert_eq!((f.stored, f.carbon), (20_000, 28_000));
        let took = tick(&mut f, &spec, &mut fuel, [false, false, true, false, false, false]);
        assert_eq!((took[2], f.carbon), (40_000, 8_000));
        let took = tick(&mut f, &spec, &mut fuel, [false, false, true, false, false, false]);
        assert_eq!((took[2], f.carbon, f.carbon_max), (8_000, 0, 0));
    }

    #[test]
    fn extraction_is_limited_by_rate_and_stock_and_simulating_changes_nothing() {
        let spec = spec(Tier::Starter);
        let mut f = Furnator { stored: 1000, ..Default::default() };
        assert_eq!(f.extract(&spec, 100, true), 100);
        assert_eq!(f.stored, 1000);
        assert_eq!(f.extract(&spec, 100, false), 100);
        assert_eq!(f.stored, 900);
        assert_eq!(f.extract(&spec, 10_000, false), 240);
        assert_eq!(f.stored, 660);
        f.stored = 7;
        assert_eq!(f.extract(&spec, 10_000, false), 7);
        assert_eq!(f.extract(&spec, 10_000, false), 0);
        assert_eq!(f.extract(&spec, -3, false), 0);
    }

    #[test]
    fn a_smaller_capacity_than_it_holds_makes_nothing_and_loses_nothing() {
        let spec = spec(Tier::Starter);
        let mut f = Furnator { stored: 30_000, carbon: 100, carbon_max: 100, burning: true };
        assert_eq!(f.generate(&spec), 0);
        assert_eq!(f, Furnator { stored: 30_000, carbon: 100, carbon_max: 100, burning: false });
    }

    #[test]
    fn slots_take_fuel_only_in_the_fuel_slot() {
        assert!(slot_accepts(FUEL_SLOT, true));
        assert!(!slot_accepts(FUEL_SLOT, false));
        assert!(!slot_accepts(CHARGE_SLOT, true));
        assert!(!slot_accepts(2, true));
    }

    #[test]
    fn the_view_shows_the_tier_and_the_fuel() {
        let view = Furnator { stored: 5, carbon: 40, carbon_max: 3000, burning: true }.view(&spec(Tier::Basic));
        assert_eq!((view.stored, view.capacity, view.generation, view.transfer), (5, 80_000, 160, 480));
        assert_eq!((view.fuel_left, view.fuel_total, view.burning), (40, 3000, true));
    }
}
