//! The Generator T1 (`GeneratorT1BE`): it burns furnace fuel from its one slot into its Forge Energy buffer, faster
//! than a furnace by the config's multiplier and by the fuel's own (the mod's coals and coal blocks: 2, 4, 8, 16), and
//! offers the buffer to its six neighbours every tick. It takes no energy from outside (`EnergyStorageNoReceive`).
use crate::{SIDES, config::GeneratorSettings, redstone::Redstone};

/// The fuel in the slot: its furnace burn time and its burn speed multiplier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuel {
    pub burn_time: i32,
    pub multiplier: i32,
}

/// The fuel slot, as the generator reads and uses it.
pub trait FuelSlot {
    /// The fuel there now; none when the slot is empty.
    fn peek(&self) -> Option<Fuel>;
    /// Uses one: the remainder replaces it (a lava bucket's bucket), else the stack shrinks by one.
    fn consume(&mut self);
}

/// The multiplier of the mod's own fuels (`Coal_T1..4.getBurnSpeedMultiplier`, `CoalBlock_T1..4`'s); 1 for any other.
/// The Fuel Canister's, which its contents decide, isn't ported.
pub fn fuel_multiplier(item: &str) -> i32 {
    match item {
        "justdirethings:coal_t1" | "justdirethings:coalblock_t1" => 2,
        "justdirethings:coal_t2" | "justdirethings:coalblock_t2" => 4,
        "justdirethings:coal_t3" | "justdirethings:coalblock_t3" => 8,
        "justdirethings:coal_t4" | "justdirethings:coalblock_t4" => 16,
        _ => 1,
    }
}

/// The block entity's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generator {
    /// The buffer (`ENERGYSTORAGE_GENERATORS`).
    pub energy: i32,
    /// Ticks the fuel burning now lasts (`maxBurn`), for the screen's flame.
    pub max_burn: i32,
    /// Ticks of it left (`burnRemaining`).
    pub burn_remaining: i32,
    /// FE it still makes (`feRemaining`).
    pub fe_remaining: i32,
    /// The burning fuel's multiplier (`fuelBurnMultiplier`).
    pub fuel_multiplier: i32,
    pub redstone: Redstone,
}

impl Default for Generator {
    fn default() -> Self {
        Self { energy: 0, max_burn: 0, burn_remaining: 0, fe_remaining: 0, fuel_multiplier: 1, redstone: Redstone::default() }
    }
}

impl Generator {
    /// `getBurnSpeedMultiplier`.
    pub fn burn_speed(&self, s: &GeneratorSettings) -> i32 {
        s.burn_speed_multiplier.wrapping_mul(self.fuel_multiplier)
    }

    /// `fePerTick`: FE the fuel burning now makes each tick.
    pub fn fe_per_tick(&self, s: &GeneratorSettings) -> i32 {
        s.fe_per_fuel_tick.wrapping_mul(self.burn_speed(s))
    }

    /// `forceReceiveEnergy`: what fits in the buffer.
    fn insert(&mut self, s: &GeneratorSettings, amount: i32, simulate: bool) -> i32 {
        let taken = (s.max_fe - self.energy).min(s.max_fe.min(amount)).max(0);
        if !simulate {
            self.energy += taken;
        }
        taken
    }

    /// `extractEnergy`: what the buffer gives of `amount`.
    pub fn extract(&mut self, s: &GeneratorSettings, amount: i64, simulate: bool) -> i64 {
        let given = i64::from(self.energy).min(i64::from(s.max_fe).min(amount)).max(0);
        if !simulate {
            self.energy -= given as i32;
        }
        given
    }

    /// `doBurn`: starts the next fuel when nothing burns and the buffer has room.
    fn burn(&mut self, s: &GeneratorSettings, slot: &mut dyn FuelSlot) {
        if self.fe_remaining > 0 {
            return;
        }
        self.max_burn = 0;
        self.burn_remaining = 0;
        if self.insert(s, self.fe_per_tick(s), true) <= 0 {
            return;
        }
        let Some(fuel) = slot.peek() else { return };
        if fuel.burn_time <= 0 {
            return;
        }
        self.fuel_multiplier = fuel.multiplier;
        slot.consume();
        self.fe_remaining = fuel.burn_time.wrapping_mul(s.fe_per_fuel_tick);
        // `(int) (Math.floor(burnTime) / getBurnSpeedMultiplier())`.
        self.max_burn = (f64::from(fuel.burn_time) / f64::from(self.burn_speed(s))) as i32;
        self.burn_remaining = self.max_burn + 1;
    }

    /// `doGenerate`: burns (where the redstone lets it) and fills the buffer, going on into the next fuel within the
    /// tick when the last one ran out short.
    pub fn generate(&mut self, s: &GeneratorSettings, slot: &mut dyn FuelSlot) {
        if self.redstone.is_active() && self.fe_remaining == 0 {
            self.burn(s, slot);
        }
        if self.fe_remaining == 0 {
            return;
        }
        let amount = self.fe_per_tick(s).min(self.fe_remaining);
        self.insert(s, amount, false);
        self.fe_remaining -= amount;
        self.burn_remaining -= 1;
        if self.redstone.is_active() && self.fe_remaining == 0 {
            self.burn(s, slot);
            if amount < self.fe_per_tick(s) {
                let more = (self.fe_per_tick(s) - amount).min(self.fe_remaining);
                self.insert(s, more, false);
                self.fe_remaining -= more;
                self.burn_remaining -= 1;
            }
        }
    }

    /// `providePowerAdjacent`: each side in turn is offered ten times the config's FE a tick, as much as it would
    /// take is drawn and handed to it. `offer(side, amount, simulate)` is that side's neighbour's `receiveEnergy`
    /// (0 where there is none).
    pub fn push(&mut self, s: &GeneratorSettings, mut offer: impl FnMut(usize, i64, bool) -> i64) {
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

    /// A slot of `count` fuels of one kind.
    struct Slot {
        fuel: Fuel,
        count: u32,
    }

    impl FuelSlot for Slot {
        fn peek(&self) -> Option<Fuel> {
            (self.count > 0).then_some(self.fuel)
        }
        fn consume(&mut self) {
            self.count -= 1;
        }
    }

    const S: GeneratorSettings = GeneratorSettings { fe_per_fuel_tick: 15, burn_speed_multiplier: 4, max_fe: 1_000_000, fe_per_tick: 1000 };
    const COAL: Fuel = Fuel { burn_time: 1600, multiplier: 1 };

    #[test]
    fn coal_burns_in_400_ticks_at_60_fe_a_tick() {
        let mut g = Generator::default();
        let mut slot = Slot { fuel: COAL, count: 1 };
        g.generate(&S, &mut slot);
        assert_eq!(slot.count, 0);
        assert_eq!((g.max_burn, g.burn_remaining, g.fe_remaining), (400, 400, 24_000 - 60));
        assert_eq!(g.energy, 60);
        let mut ticks = 1;
        while g.fe_remaining > 0 {
            g.generate(&S, &mut slot);
            ticks += 1;
        }
        assert_eq!(ticks, 400);
        assert_eq!(g.energy, 24_000, "1,600 burn ticks at 15 FE");
        // The last tick looks for the next fuel at once (`doBurn` clears the flame first) and finds none.
        assert_eq!((g.max_burn, g.burn_remaining), (0, 0), "an empty slot clears the flame");
    }

    #[test]
    fn the_mods_coal_burns_faster_and_hotter() {
        assert_eq!(fuel_multiplier("justdirethings:coal_t2"), 4);
        assert_eq!(fuel_multiplier("justdirethings:coalblock_t4"), 16);
        assert_eq!(fuel_multiplier("minecraft:coal"), 1);
        let mut g = Generator::default();
        let mut slot = Slot { fuel: Fuel { burn_time: 3200, multiplier: 2 }, count: 1 };
        g.generate(&S, &mut slot);
        assert_eq!(g.fuel_multiplier, 2);
        assert_eq!(g.fe_per_tick(&S), 120);
        assert_eq!(g.max_burn, 400, "3,200 ticks at 4 x 2");
        assert_eq!(g.energy, 120);
    }

    #[test]
    fn a_fuel_that_runs_out_mid_tick_tops_up_from_the_next() {
        // 100 burn ticks make 1,500 FE: 25 ticks of 60; a fuel of 1,510 FE (odd burn time) ends short.
        let mut g = Generator { fe_remaining: 20, burn_remaining: 1, max_burn: 25, ..Generator::default() };
        let mut slot = Slot { fuel: Fuel { burn_time: 100, multiplier: 1 }, count: 2 };
        g.generate(&S, &mut slot);
        assert_eq!(slot.count, 1, "the next fuel starts in the same tick");
        assert_eq!(g.energy, 60, "20 from the old fuel and 40 from the new");
        assert_eq!(g.fe_remaining, 1500 - 40);
        assert_eq!(g.burn_remaining, 25 + 1 - 1);
    }

    #[test]
    fn a_full_buffer_burns_nothing_and_redstone_can_stop_it() {
        let mut g = Generator { energy: S.max_fe, ..Generator::default() };
        let mut slot = Slot { fuel: COAL, count: 1 };
        g.generate(&S, &mut slot);
        assert_eq!(slot.count, 1);
        let mut g = Generator::default();
        g.redstone.mode = Mode::High;
        g.redstone.evaluate(false);
        g.generate(&S, &mut slot);
        assert_eq!((slot.count, g.energy), (1, 0));
        g.redstone.evaluate(true);
        g.generate(&S, &mut slot);
        assert_eq!((slot.count, g.energy), (0, 60));
    }

    #[test]
    fn it_offers_each_side_ten_times_its_rate() {
        let mut g = Generator { energy: 25_000, ..Generator::default() };
        let mut got = [0_i64; SIDES];
        let mut asked = Vec::new();
        g.push(&S, |side, amount, simulate| {
            if simulate {
                asked.push((side, amount));
            }
            // Only up and north take anything; north takes no more than 7,000.
            let room = match side { 1 => i64::MAX, 2 => 7_000, _ => 0 };
            let taken = amount.min(room - got[side]).max(0);
            if !simulate {
                got[side] += taken;
            }
            taken
        });
        assert_eq!(asked.len(), SIDES);
        assert!(asked.iter().all(|(_, a)| *a == 10_000));
        assert_eq!(got, [0, 10_000, 7_000, 0, 0, 0]);
        assert_eq!(g.energy, 8_000);
        let mut empty = Generator::default();
        empty.push(&S, |_, _, _| panic!("an empty generator offers nothing"));
    }
}
