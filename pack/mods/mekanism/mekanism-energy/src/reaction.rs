//! Mekanism's Pressurized Reaction Chamber, Isotopic Centrifuge and Solar
//! Neutron Activator (Mekanism 10.7.19, MIT, Copyright (c) 2017-2025 Aidan C.
//! Brady).
//!
//! The chamber (`TileEntityPressurizedReactionChamber`, a
//! `TileEntityProgressMachine`) takes an item, a fluid and a chemical and makes
//! an item and a chemical (`PressurizedReactionCachedRecipe`) over the
//! recipe's `duration` (100 until a recipe is cached), using its base Joules
//! plus the recipe's `energy_required` a tick (`PRCEnergyContainer`), both
//! under its upgrades; it runs `crate::machine::process` with one operation.
//! The centrifuge and the activator turn a chemical into another
//! (`OneInputCachedRecipe.chemicalToChemical`): the centrifuge `2^speed`
//! operations a tick on its Joules, the activator at the rate its sky gives it
//! (`recalculateProductionRate`), no Joules.
use crate::chemical::{ChemicalStack, ChemicalTank};
use crate::container::{Automation, EnergyContainer};
use crate::ore::{self, Operated};
use crate::separator;
use crate::solar::SolarCheck;

/// Each of the chamber's chemical tanks (`getInitialChemicalTanks`).
pub const PRC_TANK: i64 = 10_000;
/// The chamber's fluid tank (`getInitialFluidTanks`).
pub const PRC_FLUID: i32 = 10_000;
/// `BASE_DURATION`: the chamber's ticks before a recipe is cached.
pub const PRC_TICKS: u32 = 100;
/// The centrifuge's and the activator's tanks.
pub const TANK: i64 = 10_000;

/// A chemical-to-chemical recipe: the input's mB an operation and the output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converting {
    pub input: i64,
    pub output: ChemicalStack,
}

/// One tick of `ChemicalToChemicalCachedRecipe`: as many operations as the
/// baseline, the Joules (`per_tick` each), the input and the output's room
/// allow.
pub fn convert(recipe: &Converting, input: &mut ChemicalTank, output: &mut ChemicalTank, energy: &mut EnergyContainer, per_tick: u64, baseline: u32) -> Operated {
    let fits = if recipe.input <= 0 { 0 } else { (input.stored() / recipe.input).clamp(0, i64::from(i32::MAX)) as u32 };
    let ran = ore::operate(energy, per_tick, baseline, &[fits], separator::room_for(output, &recipe.output));
    let n = i64::from(ran.operations);
    if n > 0 {
        input.shrink_stack(recipe.input.saturating_mul(n), false);
        let made = recipe.output.copy_with_amount(recipe.output.amount().saturating_mul(n));
        output.insert(&made, false, Automation::Internal, &separator::output_rules());
    }
    ran
}

/// `recheckSettings`: the activator's peak, `maxSolarNeutronActivatorRate`
/// times the biome's factor (the solar generator's `SolarCheck.peak`).
pub fn sna_peak(max_rate: i32, check: &SolarCheck) -> f32 {
    max_rate as f32 * check.peak
}

/// `recalculateProductionRate`: none unless it sees the sun from the block
/// above it (`canSeeSun`), else the peak times the sun's brightness, a fifth
/// of that in rain where the biome has any.
pub fn sna_rate(peak: f32, check: &SolarCheck, sees_sun: bool, brightness: f32, raining: bool) -> f32 {
    if !sees_sun {
        return 0.0;
    }
    let rate = peak * brightness;
    if check.needs_rain_check && raining { rate * 0.2_f32 } else { rate }
}

/// The activator's `setRequiredTicks`: below 1 mB a tick, `ceil(1 / rate)`
/// ticks an operation, else one.
pub fn sna_ticks(rate: f32) -> u32 {
    if rate > 0.0 && rate < 1.0 { (1.0_f32 / rate).ceil() as u32 } else { 1 }
}

/// The activator's `setBaselineMaxOperations`: below 1 mB a tick one, else
/// the rate cut to a whole number (none at 0).
pub fn sna_operations(rate: f32) -> u32 {
    if rate > 0.0 && rate < 1.0 { 1 } else { rate as u32 }
}

/// The activator's tick with a recipe (`CachedRecipe.process` with required
/// ticks): `progress` counts the ticks of an operation (not saved: the
/// block entity keeps none); the input is taken and the output made when it
/// reaches `ticks`.
pub fn activate(recipe: &Converting, input: &mut ChemicalTank, output: &mut ChemicalTank, progress: &mut u32, rate: f32) -> Operated {
    let baseline = sna_operations(rate);
    let ticks = sna_ticks(rate);
    let fits = if recipe.input <= 0 { 0 } else { (input.stored() / recipe.input).clamp(0, i64::from(i32::MAX)) as u32 };
    let mut free = EnergyContainer::basic(0);
    let mut ran = ore::operate(&mut free, 0, baseline, &[fits], separator::room_for(output, &recipe.output));
    if ran.operations == 0 {
        return ran;
    }
    *progress += 1;
    if *progress < ticks {
        // Running, not finished: nothing taken or made yet.
        return ran;
    }
    *progress = 0;
    let n = i64::from(ran.operations);
    input.shrink_stack(recipe.input.saturating_mul(n), false);
    let made = recipe.output.copy_with_amount(recipe.output.amount().saturating_mul(n));
    output.insert(&made, false, Automation::Internal, &separator::output_rules());
    ran.energy = 0;
    ran
}

/// The chamber's base Joules a tick before its upgrades
/// (`PRCEnergyContainer.getBaseEnergyPerTick`): the config's plus the cached
/// recipe's `energy_required`.
pub fn prc_base_energy(usage: u64, recipe_energy: u64) -> u64 {
    usage.saturating_add(recipe_energy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chemical::ChemicalTank;

    fn tank(id: &str, amount: i64) -> ChemicalTank {
        let mut t = ChemicalTank::new(TANK);
        if amount > 0 {
            t.stored = ChemicalStack::new(id, amount);
        }
        t
    }

    #[test]
    fn the_activator_runs_at_its_sky_rate() {
        let plains = SolarCheck::new(0.8, 0.4, true);
        let peak = sna_peak(256, &plains);
        // Plains (0.8, downfall 0.4): 256 * (1 - 0.3 * 0.4).
        assert!((peak - 225.28).abs() < 1e-3, "{peak}");
        // Noon in clear plains: 225 operations a tick; rain: a fifth.
        assert_eq!(sna_operations(sna_rate(peak, &plains, true, 1.0, false)), 225);
        assert_eq!(sna_operations(sna_rate(peak, &plains, true, 1.0, true)), 45);
        // A desert never needs the rain check; no sun, nothing.
        let desert = SolarCheck::new(2.0, 0.0, false);
        assert!(sna_rate(sna_peak(256, &desert), &desert, true, 1.0, true) > 100.0);
        assert_eq!(sna_rate(peak, &plains, false, 1.0, false), 0.0);
        assert_eq!((sna_ticks(0.25), sna_operations(0.25)), (4, 1));
        assert_eq!((sna_ticks(0.0), sna_operations(0.0)), (1, 0));
    }

    #[test]
    fn the_activator_takes_its_input_when_an_operation_ends() {
        let r = Converting { input: 5, output: ChemicalStack::new("mekanism:polonium", 1) };
        let (mut i, mut o, mut p) = (tank("mekanism:nuclear_waste", 20), ChemicalTank::new(TANK), 0);
        let ran = activate(&r, &mut i, &mut o, &mut p, 0.5);
        assert_eq!((ran.operations, i.stored(), o.stored(), p), (1, 20, 0, 1));
        activate(&r, &mut i, &mut o, &mut p, 0.5);
        assert_eq!((i.stored(), o.stored(), p), (15, 1, 0));
        let ran = activate(&r, &mut i, &mut o, &mut p, 10.7);
        assert_eq!((ran.operations, i.stored(), o.stored()), (3, 0, 4));
    }

    #[test]
    fn the_centrifuge_runs_two_to_the_speed_on_its_joules() {
        let r = Converting { input: 5, output: ChemicalStack::new("mekanism:plutonium", 1) };
        let (mut i, mut o) = (tank("mekanism:nuclear_waste", 100), ChemicalTank::new(TANK));
        let mut e = EnergyContainer::basic(80_000);
        e.stored = 1000;
        let ran = convert(&r, &mut i, &mut o, &mut e, 200, 8);
        assert_eq!((ran.operations, ran.energy, i.stored(), o.stored(), e.stored), (5, 1000, 75, 5, 0));
        assert!(convert(&r, &mut i, &mut o, &mut e, 200, 8).short_energy);
        assert_eq!(prc_base_energy(5, 1000), 1005);
    }
}
