//! Mekanism's ore chain past the enrichment chamber: the Chemical Dissolution
//! Chamber (`TileEntityChemicalDissolutionChamber`, an ore and sulfuric acid
//! into a dirty slurry), the Chemical Washer (`TileEntityChemicalWasher`, a
//! dirty slurry and water into a clean one) and the Chemical Crystallizer
//! (`TileEntityChemicalCrystallizer`, a clean slurry into crystals). All three
//! run on the machines' energy and upgrades (`crate::machine`).
//!
//! - The dissolution chamber is a progress machine of 100 ticks with an input
//!   tank of 10 000 mB (`BasicChemicalTank.inputModern`, what a recipe takes
//!   with the item in its input slot) and an output tank of 10 000 mB
//!   (`BasicChemicalTank.output`). A `per_tick_usage` recipe (all of ATM10's)
//!   takes its chemical every tick (`ItemStackConstantChemicalToObjectCachedRecipe.dissolution`,
//!   `crate::advanced::process`): its amount times `StatUtils.inversePoisson`
//!   of `getGasPerTickMeanMultiplier` (`16^(2 speed/8 - chemical/8)` on ATM10),
//!   always, whatever `randomizedConsumption` says. Its output goes into the
//!   output tank on the last tick.
//! - The washer is a recipe machine (no progress): `2^speed` operations a tick
//!   (`baselineMaxOperations`) of `chemicalWasher` Joules each, as far as the
//!   Joules, the water tank (10 000 mB), the slurry tank and the output's room
//!   allow (`TwoInputCachedRecipe.fluidChemicalToChemical`).
//! - The crystallizer is a progress machine of 200 ticks
//!   (`OneInputCachedRecipe.crystallizing`): an input tank of 10 000 mB, the
//!   recipe's amount taken with the crystals on the last tick.
use crate::advanced::{self, ChemicalUse};
use crate::chemical::{ChemicalStack, ChemicalTank};
use crate::container::{Automation, EnergyContainer};
use crate::machine::{Inputs, Upgrades};
use crate::separator;

/// `TileEntityChemicalDissolutionChamber.MAX_CHEMICAL`: each of its tanks.
pub const DISSOLUTION_TANK: i64 = 10_000;
/// `TileEntityChemicalDissolutionChamber.BASE_TICKS_REQUIRED`.
pub const DISSOLUTION_TICKS: u32 = 100;
/// `TileEntityChemicalWasher.MAX_SLURRY`: each of its chemical tanks.
pub const WASHER_TANK: i64 = 10_000;
/// `TileEntityChemicalWasher.MAX_FLUID`: its water tank, in mB.
pub const WASHER_FLUID: i32 = 10_000;
/// `TileEntityChemicalCrystallizer.MAX_CHEMICAL`.
pub const CRYSTALLIZER_TANK: i64 = 10_000;
/// `TileEntityChemicalCrystallizer.BASE_TICKS_REQUIRED`.
pub const CRYSTALLIZER_TICKS: u32 = 200;

/// `injectUsageMultiplier`: `inversePoisson(injectUsage)` with `injectUsage`
/// `getGasPerTickMeanMultiplier` (the chamber takes chemical upgrades).
pub fn dissolution_use(upgrades: &Upgrades, multiplier: f64) -> ChemicalUse {
    ChemicalUse::Statistical(advanced::mean_multiplier(upgrades, true, multiplier))
}

/// A recipe's chemical input as `Inputs` (`calculateOperationsCanSupport` of a
/// chemical tank): `stored` of the recipe's chemical, `amount` an operation.
pub fn chemical_inputs(stored: i64, amount: i64) -> Inputs {
    if stored <= 0 || amount <= 0 {
        return Inputs::Mismatched;
    }
    match stored / amount {
        0 => Inputs::Short,
        n => Inputs::Enough(n.min(i64::from(u32::MAX)) as u32),
    }
}

/// What one tick of a machine without progress did (the washer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Operated {
    pub operations: u32,
    /// Joules taken.
    pub energy: u64,
    pub short_energy: bool,
    /// The first input (`NOT_ENOUGH_INPUT`).
    pub short_input: bool,
    /// The second input (`NOT_ENOUGH_SECONDARY_INPUT`).
    pub short_secondary: bool,
    pub full: bool,
}

/// `CachedRecipe.process` of a recipe machine: as many operations as the
/// baseline, the Joules (`per_tick` each), each input (`inputs`: how many
/// operations each supports) and the output's room allow, checked in that
/// order; the Joules are taken here.
pub fn operate(energy: &mut EnergyContainer, per_tick: u64, baseline: u32, inputs: &[u32], room: u32) -> Operated {
    let mut out = Operated::default();
    let mut max = baseline;
    if per_tick != 0 {
        let affordable = (energy.stored / per_tick).min(i32::MAX as u64) as u32;
        if affordable == 0 {
            out.short_energy = true;
            return out;
        }
        max = max.min(affordable);
    }
    for (i, &supports) in inputs.iter().enumerate() {
        if supports == 0 {
            if i == 0 {
                out.short_input = true;
            } else {
                out.short_secondary = true;
            }
            return out;
        }
        max = max.min(supports);
    }
    if room == 0 {
        out.full = true;
        return out;
    }
    out.operations = max.min(room);
    out.energy = energy.extract(per_tick.saturating_mul(u64::from(out.operations)), false, Automation::Internal);
    out
}

/// `fluidChemicalToChemical` of the washer's recipe: water (`fluid` mB an
/// operation) and a slurry (`slurry` mB) into `output`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Washing {
    pub fluid: i64,
    pub slurry: i64,
    pub output: ChemicalStack,
}

/// One tick of the washer: `water` is the mB of the recipe's fluid in its
/// tank, which loses what the operations take; the slurry tank and the output
/// tank change here.
pub fn wash(
    recipe: &Washing,
    water: &mut i64,
    input: &mut ChemicalTank,
    output: &mut ChemicalTank,
    energy: &mut EnergyContainer,
    per_tick: u64,
    baseline: u32,
) -> Operated {
    let fits = |stored: i64, each: i64| if each <= 0 { 0 } else { (stored / each).clamp(0, i64::from(i32::MAX)) as u32 };
    // `fluidInputHandler` is the recipe's first input, the slurry its second;
    // the washer's errors name the slurry `NOT_ENOUGH_INPUT`.
    let inputs = [fits(*water, recipe.fluid), fits(input.stored(), recipe.slurry)];
    let ran = operate(energy, per_tick, baseline, &inputs, separator::room_for(output, &recipe.output));
    let n = i64::from(ran.operations);
    if n > 0 {
        *water -= recipe.fluid.saturating_mul(n);
        input.shrink_stack(recipe.slurry.saturating_mul(n), false);
        let made = recipe.output.copy_with_amount(recipe.output.amount().saturating_mul(n));
        output.insert(&made, false, Automation::Internal, &separator::output_rules());
    }
    ran
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{self, Upgrade};

    const ATM10: f64 = 16.0;
    const DIRTY: &str = "mekanism:dirty_iron";
    const CLEAN: &str = "mekanism:clean_iron";

    fn joules(stored: u64, capacity: u64) -> EnergyContainer {
        let mut c = EnergyContainer::basic(capacity);
        c.stored = stored;
        c
    }

    fn with(speed: u32, energy: u32, chemical: u32) -> Upgrades {
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, speed);
        u.set(Upgrade::Energy, energy);
        u.set(Upgrade::Chemical, chemical);
        u
    }

    #[test]
    fn dissolution_draws_its_acid_at_random_and_fills_its_output_on_the_hundredth_tick() {
        // Measured on the NeoForge oracle (chem-work/m5 c1, D1): two iron ores, 10 000 mB of sulfuric acid,
        // 160 000 J: 400 J a tick, 0 to 3 mB of acid a tick (91 in 101 ticks), 1000 mB of dirty iron on the 100th.
        assert_eq!(dissolution_use(&Upgrades::default(), ATM10), ChemicalUse::Statistical(1.0));
        let draws = [0, 0, 1, 3, 1, 3, 1, 1, 1, 0];
        let (mut progress, mut used, mut e) = (0, 0, joules(160_000, 160_000));
        let mut acid = 10_000i64;
        let out = ChemicalTank::new(DISSOLUTION_TANK);
        let dirty = ChemicalStack::new(DIRTY, 1000);
        for t in 0..100 {
            let draw = draws.get(t).copied().unwrap_or(1);
            let room = separator::room_for(&out, &dirty);
            let ran = advanced::process(&mut progress, &mut used, DISSOLUTION_TICKS, &mut e, 400, 1, Inputs::of(2, 1), (acid, 1), draw, room);
            acid -= ran.chemical;
            if t == 2 {
                assert_eq!((progress, acid, used), (3, 9999, 1), "the oracle's third tick");
            }
            assert_eq!(ran.finished, t == 99);
        }
        assert_eq!((progress, used, e.stored), (0, 0, 120_000));
        // 8 speed and 8 energy upgrades (D2): 6 ticks of 6400 J, 256 mB a tick on average.
        let u = with(8, 8, 0);
        assert_eq!((u.ticks(DISSOLUTION_TICKS, ATM10), u.energy_per_tick(400, ATM10)), (6, 6400));
        assert_eq!(dissolution_use(&u, ATM10), ChemicalUse::Statistical(256.0));
        // 8 chemical upgrades (D3): 1/16 on average (9 in 100 ticks).
        assert_eq!(dissolution_use(&with(0, 0, 8), ATM10), ChemicalUse::Statistical(0.0625));
        // No acid (D4): the progress waits.
        let (mut progress, mut used) = (50, 0);
        let ran = advanced::process(&mut progress, &mut used, DISSOLUTION_TICKS, &mut e, 400, 1, Inputs::of(2, 1), (0, 1), 1, 1);
        assert!(ran.short_chemical);
        assert_eq!(progress, 50);
    }

    #[test]
    fn the_washer_runs_its_operations_a_tick() {
        // W1: 10 000 mB of water, 1000 of dirty iron, 80 000 J: 1 mB of clean iron a tick for 200 J, 5 of water.
        let recipe = Washing { fluid: 5, slurry: 1, output: ChemicalStack::new(CLEAN, 1) };
        let mut water = 10_000;
        let (mut input, mut output, mut e) = (ChemicalTank::with(WASHER_TANK, ChemicalStack::new(DIRTY, 1000)), ChemicalTank::new(WASHER_TANK), joules(80_000, 80_000));
        for t in 1..=3 {
            let ran = wash(&recipe, &mut water, &mut input, &mut output, &mut e, 200, separator::speed(0).0);
            assert_eq!(ran.operations, 1);
            assert_eq!((water, input.stored(), output.stored(), e.stored), (10_000 - 5 * t, 1000 - t, t, 80_000 - 200 * t as u64));
        }
        // W2: 3 speed upgrades: 8 operations of 1600 J.
        let u = with(3, 0, 0);
        let per_tick = u.energy_per_tick(200, ATM10);
        assert_eq!(per_tick, 1600);
        let mut water = 10_000;
        let (mut input, mut output, mut e) = (ChemicalTank::with(WASHER_TANK, ChemicalStack::new(DIRTY, 1000)), ChemicalTank::new(WASHER_TANK), joules(80_000, 80_000));
        wash(&recipe, &mut water, &mut input, &mut output, &mut e, per_tick, separator::speed(3).0);
        assert_eq!((water, input.stored(), output.stored(), e.stored), (9960, 992, 8, 67_200));
        // W4: 8 speed: 256 operations of 51 200 J, one as far as 80 000 J go, then none.
        let per_tick = with(8, 0, 0).energy_per_tick(200, ATM10);
        let (mut input, mut output, mut e) = (ChemicalTank::with(WASHER_TANK, ChemicalStack::new(DIRTY, 1000)), ChemicalTank::new(WASHER_TANK), joules(80_000, 80_000));
        let mut water = 10_000;
        assert_eq!(wash(&recipe, &mut water, &mut input, &mut output, &mut e, per_tick, 256).operations, 1);
        assert_eq!((e.stored, output.stored()), (28_800, 1));
        assert!(wash(&recipe, &mut water, &mut input, &mut output, &mut e, per_tick, 256).short_energy);
        // Short of water, of slurry, or of room: none.
        let mut little = 4;
        assert!(wash(&recipe, &mut little, &mut input, &mut output, &mut joules(80_000, 80_000), 200, 1).short_input);
        let mut empty = ChemicalTank::new(WASHER_TANK);
        assert!(wash(&recipe, &mut water, &mut empty, &mut output, &mut joules(80_000, 80_000), 200, 1).short_secondary);
        let mut full = ChemicalTank::with(WASHER_TANK, ChemicalStack::new(CLEAN, WASHER_TANK));
        assert!(wash(&recipe, &mut water, &mut input, &mut full, &mut joules(80_000, 80_000), 200, 1).full);
        let mut other = ChemicalTank::with(WASHER_TANK, ChemicalStack::new(DIRTY, 1));
        assert!(wash(&recipe, &mut water, &mut input, &mut other, &mut joules(80_000, 80_000), 200, 1).full, "another chemical in the output");
    }

    #[test]
    fn the_crystallizer_takes_its_slurry_with_the_crystal() {
        // C1: 1000 mB of clean iron, 160 000 J: 200 ticks of 400 J, a crystal and 200 mB taken on the 200th.
        let (mut progress, mut e) = (0, joules(160_000, 160_000));
        let mut finished = 0;
        for t in 1..=201 {
            let ran = machine::process(&mut progress, CRYSTALLIZER_TICKS, &mut e, 400, 1, chemical_inputs(1000, 200), machine::output_operations(None, 1, 64), true);
            if ran.finished {
                finished = t;
            }
        }
        assert_eq!((finished, progress, e.stored), (200, 1, 79_600));
        // C2: 8 speed and 8 energy: 12 ticks of 6400 J.
        let u = with(8, 8, 0);
        assert_eq!((u.ticks(CRYSTALLIZER_TICKS, ATM10), u.energy_per_tick(400, ATM10)), (12, 6400));
        // C3: 150 mB is short of an operation: nothing, the progress at 0.
        assert_eq!(chemical_inputs(150, 200), Inputs::Short);
        assert_eq!(chemical_inputs(0, 200), Inputs::Mismatched);
        let mut progress = 0;
        assert!(!machine::process(&mut progress, 200, &mut e, 400, 1, chemical_inputs(150, 200), 64, true).active());
    }
}
