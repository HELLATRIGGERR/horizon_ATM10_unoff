//! Mekanism's machines that make chemicals: the Chemical Oxidizer
//! (`TileEntityChemicalOxidizer`, a `TileEntityProgressMachine`: an item in,
//! 100 ticks, a chemical into a 10 000 mB output tank, `mekanism:oxidizing`)
//! and the Chemical Infuser (`TileEntityChemicalInfuser`, a
//! `TileEntityRecipeMachine`: two chemicals in, one out, `2^speed` operations
//! a tick, `mekanism:chemical_infusing`, its tanks 10 000 mB each). Both run
//! on the machines' energy and upgrades (`crate::machine`), their output tanks
//! are `BasicChemicalTank.output` (`crate::separator::output_rules`) and
//! their ejectors put the output out of its output sides at
//! `chemicalAutoEjectRate` a tick.
use crate::chemical::{ChemicalStack, ChemicalTank};
use crate::container::{Automation, EnergyContainer};
use crate::separator;

/// `TileEntityChemicalOxidizer.MAX_GAS`.
pub const OXIDIZER_TANK: i64 = 10_000;
/// `TileEntityChemicalOxidizer.BASE_TICKS_REQUIRED`.
pub const OXIDIZER_TICKS: u32 = 100;
/// `TileEntityChemicalInfuser.MAX_GAS`: each of its three tanks.
pub const INFUSER_TANK: i64 = 10_000;

/// How many operations' worth of `out` the output tank takes
/// (`OutputHelper.calculateOperationsCanSupport`).
pub fn room(tank: &ChemicalTank, out: &ChemicalStack) -> u32 {
    separator::room_for(tank, out)
}

/// `recalculateUpgrades(SPEED)`: the infuser's `baselineMaxOperations`, `2^speed`.
pub fn infuser_operations(speed: u32) -> u32 {
    separator::speed(speed).0
}

/// A chemical infusing recipe as it meets the tanks: which tank holds the
/// left ingredient (`ChemicalChemicalToChemicalCachedRecipe` swaps them when
/// the left tank holds the right one's), each side's mB an operation and the
/// output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Infusing {
    /// mB of the left tank an operation.
    pub left: i64,
    /// mB of the right tank an operation.
    pub right: i64,
    pub output: ChemicalStack,
}

/// What one tick of the infuser did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Infused {
    pub operations: u32,
    pub energy: u64,
    pub short_energy: bool,
    pub short_input: bool,
    pub full: bool,
}

/// One tick of `ChemicalChemicalToChemicalCachedRecipe.process`: as many
/// operations as the baseline, the Joules (`per_tick` each), both tanks and
/// the output's room allow, each taking both inputs and making the output.
pub fn infuse(
    recipe: &Infusing,
    left: &mut ChemicalTank,
    right: &mut ChemicalTank,
    center: &mut ChemicalTank,
    energy: &mut EnergyContainer,
    per_tick: u64,
    baseline: u32,
) -> Infused {
    let mut out = Infused::default();
    let mut max = baseline;
    if per_tick != 0 {
        let affordable = (energy.stored / per_tick).min(i32::MAX as u64) as u32;
        if affordable == 0 {
            out.short_energy = true;
            return out;
        }
        max = max.min(affordable);
    }
    let fits = |stored: i64, each: i64| if each <= 0 { 0 } else { (stored / each).clamp(0, i64::from(i32::MAX)) as u32 };
    let inputs = fits(left.stored(), recipe.left).min(fits(right.stored(), recipe.right));
    if inputs == 0 {
        out.short_input = true;
        return out;
    }
    let room = room(center, &recipe.output);
    if room == 0 {
        out.full = true;
        return out;
    }
    let operations = max.min(inputs).min(room);
    out.operations = operations;
    out.energy = energy.extract(per_tick.saturating_mul(u64::from(operations)), false, Automation::Internal);
    let n = i64::from(operations);
    left.shrink_stack(recipe.left.saturating_mul(n), false);
    right.shrink_stack(recipe.right.saturating_mul(n), false);
    let made = recipe.output.copy_with_amount(recipe.output.amount().saturating_mul(n));
    center.insert(&made, false, Automation::Internal, &separator::output_rules());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{self, Inputs, Upgrade, Upgrades};

    const H: &str = "mekanism:hydrogen";
    const CL: &str = "mekanism:chlorine";
    const HCL: &str = "mekanism:hydrogen_chloride";

    fn tank(chemical: &str, amount: i64) -> ChemicalTank {
        ChemicalTank::with(INFUSER_TANK, ChemicalStack::new(chemical, amount))
    }

    fn joules(stored: u64) -> EnergyContainer {
        let mut c = EnergyContainer::basic(80_000);
        c.stored = stored;
        c
    }

    #[test]
    fn hydrogen_and_chlorine_make_hydrogen_chloride() {
        // Measured on the NeoForge oracle (oracle4, o1): 1000 mB each, 80 000 J.
        let recipe = Infusing { left: 1, right: 1, output: ChemicalStack::new(HCL, 1) };
        let (mut l, mut r, mut c, mut e) = (tank(H, 1000), tank(CL, 1000), ChemicalTank::new(INFUSER_TANK), joules(80_000));
        for t in 1..=3 {
            let ran = infuse(&recipe, &mut l, &mut r, &mut c, &mut e, 200, infuser_operations(0));
            assert_eq!(ran.operations, 1);
            assert_eq!((l.stored(), r.stored(), c.stored(), e.stored), (1000 - t, 1000 - t, t, 80_000 - 200 * t as u64));
        }
        // Three speed upgrades: 8 operations of 1600 J.
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, 3);
        let per_tick = u.energy_per_tick(200, 16.0);
        assert_eq!((per_tick, infuser_operations(3)), (1600, 8));
        let (mut l, mut r, mut c, mut e) = (tank(H, 1000), tank(CL, 1000), ChemicalTank::new(INFUSER_TANK), joules(80_000));
        infuse(&recipe, &mut l, &mut r, &mut c, &mut e, per_tick, 8);
        assert_eq!((l.stored(), c.stored(), e.stored), (992, 8, 67_200));
        // Eight: 256 operations of 51 200 J, as far as the Joules go: one.
        u.set(Upgrade::Speed, 8);
        let (mut l, mut r, mut c, mut e) = (tank(H, 1000), tank(CL, 1000), ChemicalTank::new(INFUSER_TANK), joules(80_000));
        let ran = infuse(&recipe, &mut l, &mut r, &mut c, &mut e, u.energy_per_tick(200, 16.0), infuser_operations(8));
        assert_eq!((ran.operations, e.stored, c.stored()), (1, 28_800, 1));
        assert!(infuse(&recipe, &mut l, &mut r, &mut c, &mut e, 51_200, 256).short_energy);
    }

    #[test]
    fn a_full_output_an_empty_input_or_too_few_joules_stop_it() {
        let recipe = Infusing { left: 1, right: 1, output: ChemicalStack::new(HCL, 1) };
        let (mut l, mut r, mut c, mut e) = (tank(H, 1000), tank(CL, 1000), tank(HCL, 10_000), joules(80_000));
        assert!(infuse(&recipe, &mut l, &mut r, &mut c, &mut e, 200, 1).full);
        let (mut l, mut r, mut c, mut e) = (tank(H, 1000), ChemicalTank::new(INFUSER_TANK), ChemicalTank::new(INFUSER_TANK), joules(80_000));
        assert!(infuse(&recipe, &mut l, &mut r, &mut c, &mut e, 200, 1).short_input);
        let (mut l, mut r, mut c, mut e) = (tank(H, 1000), tank(CL, 1000), ChemicalTank::new(INFUSER_TANK), joules(150));
        assert!(infuse(&recipe, &mut l, &mut r, &mut c, &mut e, 200, 1).short_energy);
        assert_eq!((l.stored(), e.stored), (1000, 150));
    }

    #[test]
    fn the_oxidizer_runs_a_hundred_ticks_into_its_tank() {
        // o1: two sulfur dusts, 80 000 J: 200 J a tick, 100 mB of sulfur dioxide on the hundredth.
        let so2 = ChemicalStack::new("mekanism:sulfur_dioxide", 100);
        let (mut progress, mut e, tank) = (0, joules(80_000), ChemicalTank::new(OXIDIZER_TANK));
        let mut finished = 0;
        for t in 1..=101 {
            let ran = machine::process(&mut progress, OXIDIZER_TICKS, &mut e, 200, 1, Inputs::of(2, 1), room(&tank, &so2), true);
            if ran.finished {
                finished = t;
            }
        }
        assert_eq!((finished, progress, e.stored), (100, 1, 59_800));
        // 9950 mB held: no room for 100, the progress waits.
        let full = ChemicalTank::with(OXIDIZER_TANK, ChemicalStack::new("mekanism:sulfur_dioxide", 9950));
        assert_eq!(room(&full, &so2), 0);
        let mut progress = 50;
        let ran = machine::process(&mut progress, OXIDIZER_TICKS, &mut e, 200, 1, Inputs::of(2, 1), room(&full, &so2), true);
        assert!(!ran.active());
        assert_eq!(progress, 50);
        let other = ChemicalTank::with(OXIDIZER_TANK, ChemicalStack::new(H, 10));
        assert_eq!(room(&other, &so2), 0, "another chemical in the tank");
        // 8 speed and 8 energy upgrades: 6 ticks of 3200 J.
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, 8);
        u.set(Upgrade::Energy, 8);
        assert_eq!((u.ticks(OXIDIZER_TICKS, 16.0), u.energy_per_tick(200, 16.0)), (6, 3200));
    }
}
