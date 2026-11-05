//! Mekanism's advanced machines (`TileEntityAdvancedElectricMachine`): the
//! Osmium Compressor, the Purification Chamber and the Chemical Injection
//! Chamber, an item and a chemical in (a tank of 210 mB filled from its
//! chemical slot), an item out, 200 ticks a recipe. A recipe with
//! `per_tick_usage` (all of ATM10's) takes its chemical every tick
//! (`ItemStackConstantChemicalToObjectCachedRecipe`): its amount times a
//! multiplier drawn each tick (`ChemicalUsageMultiplier`).
//!
//! - The compressor's multiplier is constant (`constantUse`): what is left of
//!   `baseTotalUsage` (200) over the ticks left, so a recipe takes 200 times
//!   its amount however many speed upgrades it has; energy upgrades change
//!   nothing; it takes no chemical upgrades.
//! - The purification and injection chambers draw it at random with
//!   `randomizedConsumption` (on, the default, in ATM10): `StatUtils.inversePoisson`
//!   of `maxUpgradeMultiplier ^ (2 speed/8 - chemical/8)` a tick, so speed
//!   upgrades raise what a recipe takes (16 times more with 8 of them on ATM10,
//!   more than the 210 mB tank holds: it stalls) and chemical upgrades lower it;
//!   energy upgrades change nothing. Without it they use `constantUse` of
//!   `round(200 * multiplier ^ (speed/8 - chemical/8))`.
//!
//! A tick short of the chemical (the tank holds less than the recipe's amount,
//! or less than this tick's use) waits with its progress; `used_so_far` is
//! saved and starts over only when an operation finishes.
use crate::container::{Automation, EnergyContainer};
use crate::machine::{Inputs, Upgrade, Upgrades};

/// `TileEntityAdvancedElectricMachine.MAX_GAS`: the chemical tank, in mB.
pub const MAX_GAS: i64 = 210;
/// `BASE_TICKS_REQUIRED`.
pub const BASE_TICKS: u32 = 200;

/// `MekanismUtils.fractionUpgrades`.
fn fraction(upgrades: &Upgrades, upgrade: Upgrade) -> f64 {
    f64::from(upgrades.get(upgrade)) / f64::from(upgrade.max())
}

/// `MekanismUtils.getBaseUsage(tile, def)`: `round(def * multiplier ^ (speed/8 - chemical/8))`
/// for a machine taking chemical upgrades, else `def`.
pub fn base_usage(upgrades: &Upgrades, def: u32, takes_chemical: bool, multiplier: f64) -> i64 {
    if !takes_chemical {
        return i64::from(def);
    }
    let exponent = fraction(upgrades, Upgrade::Speed) - fraction(upgrades, Upgrade::Chemical);
    (f64::from(def) * multiplier.powf(exponent)).round() as i64
}

/// `MekanismUtils.getGasPerTickMeanMultiplier`: `multiplier ^ (2 speed/8 - chemical/8)`
/// for a machine taking chemical upgrades, else `multiplier ^ (speed/8)`.
pub fn mean_multiplier(upgrades: &Upgrades, takes_chemical: bool, multiplier: f64) -> f64 {
    let speed = fraction(upgrades, Upgrade::Speed);
    if takes_chemical { multiplier.powf(2.0 * speed - fraction(upgrades, Upgrade::Chemical)) } else { multiplier.powf(speed) }
}

/// `ChemicalUsageMultiplier.constantUse`: what is left of `base_total` spread
/// over the ticks left (none while the ticks outnumber it, one when equal).
pub fn constant_use(base_total: i64, used_so_far: i64, required: u32, operating: u32) -> i64 {
    let base_remaining = base_total - used_so_far;
    let remaining_ticks = i64::from(required) - i64::from(operating);
    if base_remaining < remaining_ticks {
        0
    } else if base_remaining == remaining_ticks {
        1
    } else {
        crate::clamp_to_long(base_remaining as f64 / remaining_ticks as f64).max(0)
    }
}

/// `StatUtils.inversePoisson(mean)` for a uniform `r` in [0, 1): Stirling's
/// approximation of the Poisson distribution's inverse, at most `3 * ceil(mean)`.
pub fn inverse_poisson(mean: f64, r: f64) -> i64 {
    let r = r * mean.exp();
    let coeff = 1.0 / (std::f64::consts::PI * 2.0).sqrt();
    let stirling = mean * std::f64::consts::E;
    let bound = 3.0 * mean.ceil();
    let (mut m, mut p) = (0i64, 1.0f64);
    while p < r && (m as f64) < bound {
        m += 1;
        p += coeff / (m as f64).sqrt() * (stirling / m as f64).powi(m as i32);
    }
    m
}

/// How a machine's chemical use a tick is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChemicalUse {
    /// `constantUse` of this base total.
    Constant(i64),
    /// `inversePoisson` of this mean.
    Statistical(f64),
}

impl ChemicalUse {
    /// `recalculateUpgrades`: the compressor's is constant; the purification
    /// and injection chambers' (chemical upgrades taken) statistical when
    /// `randomized`.
    pub fn of(upgrades: &Upgrades, takes_chemical: bool, randomized: bool, multiplier: f64) -> Self {
        if takes_chemical && randomized {
            Self::Statistical(mean_multiplier(upgrades, takes_chemical, multiplier))
        } else {
            Self::Constant(base_usage(upgrades, BASE_TICKS, takes_chemical, multiplier))
        }
    }

    /// `setupVariableValues`: this tick's multiplier, from `r` in [0, 1) for a
    /// statistical one.
    pub fn draw(self, used_so_far: i64, required: u32, operating: u32, r: f64) -> i64 {
        match self {
            Self::Constant(total) => constant_use(total, used_so_far, required, operating),
            Self::Statistical(mean) => inverse_poisson(mean, r),
        }
        .max(0)
    }
}

/// A small random source (xorshift64*) for the statistical draw: Mekanism's
/// `RandomSource.create()` is the game's, unseeded.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// A double in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// What one tick of a per-tick recipe did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    pub operations: u32,
    pub finished: bool,
    /// Joules taken.
    pub energy: u64,
    /// mB of the chemical taken.
    pub chemical: i64,
    /// Waiting for the chemical (`NOT_ENOUGH_SECONDARY_INPUT`).
    pub short_chemical: bool,
}

/// One tick of `ItemStackConstantChemicalToObjectCachedRecipe.process`:
/// short of a tick's energy it waits; an item input that no longer fits starts
/// the progress over (`used_so_far` stays); a tank without the recipe's amount,
/// or short of this tick's use (`multiplier` times the amount), and a full
/// output wait. Else it takes the energy and the tick's chemical (on the last
/// tick with the item, as `finishProcessing` does) and moves on a tick.
/// `chemical` is the tank's mB of the recipe's chemical (0 when it holds
/// another) and the recipe's amount.
#[allow(clippy::too_many_arguments)]
pub fn process(
    progress: &mut u32,
    used_so_far: &mut i64,
    required: u32,
    energy: &mut EnergyContainer,
    per_tick: u64,
    baseline: u32,
    item: Inputs,
    chemical: (i64, i64),
    multiplier: i64,
    room: u32,
) -> Ran {
    let mut max_for_energy = baseline;
    if per_tick != 0 {
        let affordable = (energy.stored / per_tick).min(i32::MAX as u64) as u32;
        if affordable == 0 {
            return Ran::default();
        }
        max_for_energy = affordable;
    }
    let item_ops = match item {
        Inputs::Mismatched | Inputs::Short => {
            *progress = 0;
            return Ran::default();
        }
        Inputs::Enough(n) => n,
    };
    let (stored, amount) = chemical;
    if amount <= 0 || stored < amount {
        return Ran { short_chemical: true, ..Ran::default() };
    }
    let mut operations = baseline.min(item_ops);
    if multiplier > 0 {
        let can = (stored / amount.saturating_mul(multiplier)).clamp(0, i64::from(i32::MAX)) as u32;
        if can == 0 {
            return Ran { short_chemical: true, ..Ran::default() };
        }
        operations = operations.min(can);
    }
    let operations = operations.min(room).min(max_for_energy);
    if operations == 0 {
        return Ran::default();
    }
    let used = energy.extract(per_tick.saturating_mul(u64::from(operations)), false, Automation::Internal);
    let take = amount.saturating_mul(multiplier).saturating_mul(i64::from(operations));
    *progress += 1;
    let finished = *progress >= required;
    if finished {
        *progress = 0;
        *used_so_far = 0;
    } else {
        *used_so_far += multiplier.saturating_mul(i64::from(operations));
    }
    Ran { operations, finished, energy: used, chemical: take, short_chemical: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATM10: f64 = 16.0;

    fn with(speed: u32, energy: u32, chemical: u32) -> Upgrades {
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, speed);
        u.set(Upgrade::Energy, energy);
        u.set(Upgrade::Chemical, chemical);
        u
    }

    fn energy(stored: u64, capacity: u64) -> EnergyContainer {
        let mut c = EnergyContainer::basic(capacity);
        c.stored = stored;
        c
    }

    /// Runs a compressor's recipe: (osmium per tick, energy after) each tick.
    fn compress(upgrades: Upgrades, tank: &mut i64, stored: u64, ticks: usize) -> (Vec<i64>, u64, u32) {
        let required = upgrades.ticks(BASE_TICKS, ATM10);
        let per_tick = upgrades.energy_per_tick(100, ATM10);
        let usage = ChemicalUse::of(&upgrades, false, true, ATM10);
        let mut e = energy(stored, upgrades.max_energy(80_000, ATM10));
        let (mut progress, mut used, mut out) = (0, 0, Vec::new());
        for _ in 0..ticks {
            let m = usage.draw(used, required, progress, 0.5);
            let ran = process(&mut progress, &mut used, required, &mut e, per_tick, 1, Inputs::of(2, 1), (*tank, 1), m, 64);
            *tank -= ran.chemical;
            out.push(ran.chemical);
        }
        (out, e.stored, progress)
    }

    #[test]
    fn the_compressor_takes_two_hundred_whatever_its_speed() {
        // Measured on the NeoForge oracle (oracle4, a1): 210 mB of osmium, two glowstone dusts, 80 000 J.
        let mut tank = 210;
        let (use_, joules, progress) = compress(Upgrades::default(), &mut tank, 80_000, 201);
        assert!(use_.iter().all(|&u| u == 1), "1 a tick, the last with the dust");
        assert_eq!((tank, joules, progress), (9, 59_900, 1));
        // 8 speed: 12 ticks of 25 600 J; 80 000 J last three of them, 16 mB each.
        let mut tank = 210;
        let (use_, joules, progress) = compress(with(8, 0, 0), &mut tank, 80_000, 13);
        assert_eq!(use_, [16, 16, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!((tank, joules, progress), (162, 3200, 3));
        // 4 speed and 8 energy: 50 ticks of 100 J, 4 mB a tick, 200 in all.
        let mut tank = 210;
        let (use_, joules, _) = compress(with(4, 8, 0), &mut tank, 80_000, 50);
        assert!(use_.iter().all(|&u| u == 4));
        assert_eq!((tank, joules), (10, 75_000));
        assert_eq!(ChemicalUse::of(&with(8, 8, 8), false, true, ATM10), ChemicalUse::Constant(200), "no chemical upgrades for it");
    }

    #[test]
    fn an_empty_tank_waits_and_keeps_the_progress() {
        let (mut progress, mut used, mut e) = (50, 50, energy(80_000, 80_000));
        let m = constant_use(200, used, 200, progress);
        let ran = process(&mut progress, &mut used, 200, &mut e, 100, 1, Inputs::of(2, 1), (0, 1), m, 64);
        assert!(ran.short_chemical && ran.operations == 0);
        assert_eq!((progress, used, e.stored), (50, 50, 80_000));
        let ran = process(&mut progress, &mut used, 200, &mut e, 100, 1, Inputs::Mismatched, (10, 1), m, 64);
        assert_eq!((ran.operations, progress, used), (0, 0, 50), "the item taken out: progress starts over, the use so far stays");
        assert_eq!(constant_use(200, 50, 200, 0), 0, "more ticks left than use: none until they meet");
        assert_eq!(constant_use(200, 50, 200, 50), 1);
    }

    #[test]
    fn the_chambers_draw_their_use_at_random() {
        // 1 a tick on average with no upgrades: 0 to 3 (`inversePoisson`'s bound).
        let mean = mean_multiplier(&Upgrades::default(), true, ATM10);
        assert_eq!(mean, 1.0);
        assert_eq!((inverse_poisson(1.0, 0.0), inverse_poisson(1.0, 0.36), inverse_poisson(1.0, 0.37)), (0, 0, 1));
        assert_eq!((inverse_poisson(1.0, 0.76), inverse_poisson(1.0, 0.9), inverse_poisson(1.0, 0.999_999)), (1, 2, 3));
        let mut rng = Rng::new(42);
        let draws: Vec<i64> = (0..20_000).map(|_| inverse_poisson(1.0, rng.next_f64())).collect();
        assert!(draws.iter().all(|d| (0..=3).contains(d)));
        let avg = draws.iter().sum::<i64>() as f64 / draws.len() as f64;
        // P(1) = 1.0844/e, P(2) = 0.5211/e, P(3) = 0.1128/e: 0.907 on average (the oracle: 186 and 165 in 200 ticks).
        assert!((avg - 0.907).abs() < 0.02, "{avg}");
        // 8 speed upgrades: a mean of 256 a tick, more than the tank holds; 8 chemical: 1/16.
        assert_eq!(mean_multiplier(&with(8, 0, 0), true, ATM10), 256.0);
        assert_eq!(mean_multiplier(&with(0, 8, 8), true, ATM10), 0.0625, "energy upgrades change nothing");
        assert_eq!(ChemicalUse::of(&with(8, 0, 0), true, true, ATM10), ChemicalUse::Statistical(256.0));
        assert_eq!(ChemicalUse::of(&with(8, 0, 0), true, false, ATM10), ChemicalUse::Constant(3200), "not randomized: base usage");
        assert_eq!(base_usage(&with(0, 0, 8), 200, true, ATM10), 13);
        let (mut progress, mut used, mut e) = (0, 0, energy(1_000_000, 1_280_000));
        let ran = process(&mut progress, &mut used, 12, &mut e, 3200, 1, Inputs::of(4, 1), (210, 1), 256, 64);
        assert!(ran.short_chemical && progress == 0, "256 mB of 210: it stalls");
        let ran = process(&mut progress, &mut used, 200, &mut e, 200, 1, Inputs::of(2, 1), (210, 1), 0, 64);
        assert_eq!((ran.operations, ran.chemical, progress), (1, 0, 1), "a draw of 0 runs and takes none");
    }
}
