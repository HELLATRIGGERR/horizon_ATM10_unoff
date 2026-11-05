//! Mekanism Generators' Gas-Burning Generator (`TileEntityGasGenerator`): a
//! fuel tank (`generators.toml` `[gas_generator] tankCapacity`, 18 000 mB)
//! that takes any chemical with a fuel attribute (`ChemicalFuel`, the
//! `chemical_attribute_fuel` data map: hydrogen burns a tick a mB for 200 J a
//! tick on ATM10), burns up to 256 of its ticks a tick, fewer as the tank
//! empties (`ceil(256 * stored / capacity)`) and no more than its store has
//! room for, and holds 1000 times hydrogen's energy density. It puts out of
//! its front (`TileEntityGenerator`) twice what it made last tick, at least
//! twice hydrogen's Joules a tick. What it burns between ticks
//! (`burnTicks`, `maxBurnTicks`, `generationRate`) isn't saved.
use crate::chemical::Fuel;
use crate::container::{Automation, EnergyContainer};

/// `GeneratorsConfig.gbgTankCapacity`'s default, in mB.
pub const TANK: i64 = 18_000;
/// The most burn ticks a full tank burns in a tick (`getToUse`).
pub const MAX_BURN: f64 = 256.0;

/// `withEnergyConfig(1000 * hydrogenEnergyDensity)`: the Joules it holds.
pub fn storage(hydrogen_density: u64) -> u64 {
    hydrogen_density.saturating_mul(1000)
}

/// `updateMaxOutputRaw`: twice what it is given.
pub fn max_output(raw: u64) -> u64 {
    raw.saturating_mul(2)
}

/// What a generator keeps between ticks and doesn't save.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Burner {
    /// Ticks left of the mB being burnt.
    pub burn_ticks: i64,
    /// The fuel's ticks a mB (`maxBurnTicks`), 0 until it burns.
    pub max_burn_ticks: u32,
    /// The fuel's Joules a tick (`generationRate`), 0 until it burns.
    pub generation_rate: u64,
    /// `maxOutput`: what the ejector gives a tick, already doubled.
    pub max_output: u64,
    /// `gasUsedLastTick`: mB, a fraction.
    pub used_last_tick: f64,
}

/// What a tick of burning did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Burned {
    pub active: bool,
    /// Burn ticks used (`toUse`).
    pub ticks: u64,
    /// Joules made.
    pub produced: u64,
}

impl Burner {
    /// A generator just made or loaded: the constructor's twice hydrogen's
    /// Joules a tick, or, with a tank holding `fuel`, twice its Joules a tick
    /// (`FuelTank.recheckOutput` as the save loads into an empty tank).
    pub fn new(hydrogen_per_tick: u64, fuel: Option<Fuel>) -> Self {
        Self { max_output: max_output(fuel.map_or(hydrogen_per_tick, |f| f.energy_per_tick)), ..Self::default() }
    }

    /// `recheckOutput`: an empty tank came to hold `fuel`.
    pub fn filled(&mut self, fuel: Option<Fuel>) {
        if let Some(f) = fuel {
            self.max_output = max_output(f.energy_per_tick);
        }
    }

    /// `getToUse`: the burn ticks this tick, by how full the tank is and the
    /// room in the store.
    fn to_use(&self, stored: i64, capacity: i64, energy: &EnergyContainer) -> u64 {
        if self.generation_rate == 0 || stored <= 0 {
            return 0;
        }
        let share = (MAX_BURN * (stored as f64 / capacity.max(1) as f64)).ceil();
        let mut max = crate::clamp_to_long(share).max(0) as u64;
        let left = u64::from(self.max_burn_ticks).saturating_mul(stored as u64).saturating_add(self.burn_ticks.max(0) as u64);
        max = max.min(left);
        let room = crate::clamp_to_long(energy.needed() as f64 / self.generation_rate as f64).max(0) as u64;
        room.min(max)
    }

    /// `onUpdateServer` after the ejector and the slots: with fuel in the tank
    /// (`stored` mB of `fuel`), room for a tick's Joules and leave to run, it
    /// burns `getToUse` ticks into the store and takes the whole mB they come
    /// to out of the tank; else it rests, and an empty tank with nothing left
    /// burning starts over (`reset`).
    pub fn tick(&mut self, stored: &mut i64, capacity: i64, fuel: Option<Fuel>, energy: &mut EnergyContainer, can_function: bool, hydrogen_per_tick: u64) -> Burned {
        if *stored > 0 && can_function && energy.insert(self.generation_rate, true, Automation::Internal) == 0 {
            if let Some(f) = fuel {
                self.max_burn_ticks = f.burn_ticks.max(1);
                self.generation_rate = f.energy_per_tick;
            }
            let to_use = self.to_use(*stored, capacity, energy);
            let generation = self.generation_rate.saturating_mul(to_use);
            self.max_output = max_output(hydrogen_per_tick.max(generation));
            let per = i64::from(self.max_burn_ticks.max(1));
            let total = self.burn_ticks.saturating_add(stored.saturating_mul(per)).saturating_sub(to_use as i64);
            energy.insert(generation, false, Automation::Internal);
            *stored = (total / per).max(0);
            self.burn_ticks = total % per;
            self.used_last_tick = to_use as f64 / per as f64;
            return Burned { active: true, ticks: to_use, produced: generation };
        }
        if *stored <= 0 && self.burn_ticks == 0 {
            *self = Self { max_output: max_output(hydrogen_per_tick), ..Self::default() };
        }
        self.used_last_tick = 0.0;
        Burned::default()
    }

    /// `getUsed`: mB burnt last tick, to the hundredth.
    pub fn used(&self) -> f64 {
        (self.used_last_tick * 100.0).round() / 100.0
    }

    /// `getProductionRate`: what it made last tick, as the screen shows it.
    pub fn production(&self) -> u64 {
        crate::clamp_to_long(self.generation_rate as f64 * self.used() * f64::from(self.max_burn_ticks)).max(0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HYDROGEN: Fuel = Fuel { burn_ticks: 1, energy_per_tick: 200 };
    /// Ethene as Mekanism alone has it (the oracle's Mekanism-only server); ATM10's is 20 ticks of 300 J.
    const ETHENE: Fuel = Fuel { burn_ticks: 40, energy_per_tick: 705 };

    fn store(stored: u64) -> EnergyContainer {
        let mut c = EnergyContainer::output(storage(200));
        c.stored = stored;
        c
    }

    /// One tick as the oracle runs it: the ejector, then the burn.
    fn run(b: &mut Burner, tank: &mut i64, fuel: Fuel, energy: &mut EnergyContainer, ticks: usize) -> Vec<(u64, i64)> {
        (0..ticks)
            .map(|_| {
                b.tick(tank, TANK, (*tank > 0).then_some(fuel), energy, true, 200);
                (energy.stored, *tank)
            })
            .collect()
    }

    #[test]
    fn hydrogen_burns_by_how_full_the_tank_is() {
        // Measured on NeoForge 1.21.1 with Mekanism Generators 10.7.19 and ATM10's configs (oracle4, g1).
        assert_eq!(storage(200), 200_000);
        let (mut b, mut tank, mut e) = (Burner::new(200, Some(HYDROGEN)), 1000, store(0));
        assert_eq!(b.max_output, 400);
        assert_eq!(run(&mut b, &mut tank, HYDROGEN, &mut e, 4), [(3000, 985), (6000, 970), (8800, 956), (11_600, 942)]);
        let (mut b, mut tank, mut e) = (Burner::new(200, Some(HYDROGEN)), 18_000, store(0));
        assert_eq!(run(&mut b, &mut tank, HYDROGEN, &mut e, 2), [(51_200, 17_744), (101_800, 17_491)]);
        assert_eq!(b.max_output, 2 * 50_600, "twice what it made last tick");
        assert_eq!((b.used(), b.production()), (253.0, 50_600));
    }

    #[test]
    fn a_fuel_of_many_ticks_a_mb_keeps_what_it_has_burnt() {
        let (mut b, mut tank, mut e) = (Burner::new(200, Some(ETHENE)), 1000, store(0));
        let rows = run(&mut b, &mut tank, ETHENE, &mut e, 25);
        let tanks: Vec<i64> = rows.iter().map(|r| r.1).collect();
        assert_eq!(&tanks[..11], [999, 999, 998, 998, 998, 997, 997, 997, 996, 996, 995]);
        assert_eq!(rows[0].0, 10_575, "15 ticks of 705 J");
        assert_eq!(rows[18], (199_515, 992), "the last 9650 J of room take 13 ticks");
        assert!(rows[19..].iter().all(|r| *r == (199_515, 992)), "less room than a tick's Joules: it rests");
    }

    #[test]
    fn a_full_store_rests_and_an_empty_tank_starts_over() {
        let (mut b, mut tank, mut e) = (Burner::new(200, Some(HYDROGEN)), 1000, store(198_500));
        assert_eq!(run(&mut b, &mut tank, HYDROGEN, &mut e, 3), [(199_900, 993); 3]);
        let (mut b, mut tank, mut e) = (Burner::new(200, Some(HYDROGEN)), 1000, store(200_000));
        let first = b.tick(&mut tank, TANK, Some(HYDROGEN), &mut e, true, 200);
        assert_eq!((first.active, first.ticks, tank), (true, 0, 1000), "no rate yet: it runs and burns nothing");
        assert!(!b.tick(&mut tank, TANK, Some(HYDROGEN), &mut e, true, 200).active, "then a tick's Joules don't fit");
        assert_eq!((tank, e.stored), (1000, 200_000));
        let (mut b, mut tank, mut e) = (Burner::new(200, Some(HYDROGEN)), 1000, store(0));
        assert!(!b.tick(&mut tank, TANK, Some(HYDROGEN), &mut e, false, 200).active, "redstone off");
        let mut empty = 0;
        b.generation_rate = 200;
        b.max_output = 6000;
        b.tick(&mut empty, TANK, None, &mut e, true, 200);
        assert_eq!((b.max_output, b.generation_rate), (400, 0), "reset: twice hydrogen's");
        let mut b = Burner::new(200, None);
        b.filled(Some(ETHENE));
        assert_eq!(b.max_output, 1410);
    }
}
