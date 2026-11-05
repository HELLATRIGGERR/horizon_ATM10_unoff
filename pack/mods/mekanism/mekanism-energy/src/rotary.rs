//! Mekanism's Rotary Condensentrator (`TileEntityRotaryCondensentrator`): a
//! chemical tank and a fluid tank of 10 000 mB each, and a mode
//! (`mode`, saved as such): condensentrating (`false`, the default) turns the
//! chemical in its tank into the fluid of a `mekanism:rotary` recipe,
//! decondensentrating (`true`) the fluid back (`RotaryCachedRecipe`). It is a
//! recipe machine without progress: `2^speed` operations a tick
//! (`baselineMaxOperations`) of `rotaryCondensentrator` Joules each, as far as
//! the Joules, the input and the output's room allow
//! (`crate::ore::operate`).
//!
//! Its tanks take and give by the mode (`getInitialChemicalTanks`,
//! `getInitialFluidTanks`): condensentrating, the chemical tank takes from
//! anything and gives only to a player, the fluid tank the other way round;
//! decondensentrating, the opposite. Its chemical config is
//! `setupIOConfig(CHEMICAL, gasTank, LEFT, true)` and its fluid config
//! `setupIOConfig(FLUID, fluidTank, RIGHT, true)` (every type takes and gives,
//! the tank's rules decide), the chemical ejecting while decondensentrating and
//! the fluid while condensentrating.
use crate::container::Automation;

/// `TileEntityRotaryCondensentrator.CAPACITY`: each tank.
pub const TANK: i64 = 10_000;
pub const FLUID_TANK: i32 = 10_000;

/// The chemical tank's `canExtract`: a player, or decondensentrating.
pub fn chemical_out(mode: bool, automation: Automation) -> bool {
    automation == Automation::Manual || mode
}

/// The chemical tank's `canInsert`: the machine itself, or condensentrating.
pub fn chemical_in(mode: bool, automation: Automation) -> bool {
    automation == Automation::Internal || !mode
}

/// The fluid tank's `canExtract`: a player, or condensentrating.
pub fn fluid_out(mode: bool, automation: Automation) -> bool {
    automation == Automation::Manual || !mode
}

/// The fluid tank's `canInsert`: the machine itself, or decondensentrating.
pub fn fluid_in(mode: bool, automation: Automation) -> bool {
    automation == Automation::Internal || mode
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chemical::{ChemicalStack, ChemicalTank};
    use crate::container::EnergyContainer;
    use crate::machine::{Upgrade, Upgrades};
    use crate::ore::operate;
    use crate::separator;

    fn joules(stored: u64) -> EnergyContainer {
        let mut c = EnergyContainer::basic(20_000);
        c.stored = stored;
        c
    }

    #[test]
    fn it_condensentrates_and_decondensentrates_a_mb_an_operation() {
        // Measured on the NeoForge oracle (chem-work/m5 c1, R1-R3): 1000 mB of hydrogen, 20 000 J: 1 mB of liquid
        // hydrogen a tick for 50 J; 3 speed upgrades 8 a tick of 400 J each; the other way 1 a tick.
        let mut e = joules(20_000);
        let mut gas = ChemicalTank::with(TANK, ChemicalStack::new("mekanism:hydrogen", 1000));
        let ran = operate(&mut e, 50, separator::speed(0).0, &[gas.stored() as u32], 10_000);
        assert_eq!((ran.operations, e.stored), (1, 19_950));
        gas.shrink_stack(1, false);
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, 3);
        let per_tick = u.energy_per_tick(50, 16.0);
        assert_eq!(per_tick, 400);
        let mut e = joules(20_000);
        let ran = operate(&mut e, per_tick, separator::speed(3).0, &[999], 10_000);
        assert_eq!((ran.operations, e.stored), (8, 16_800));
        // A full fluid tank, or too few Joules: none.
        assert!(operate(&mut joules(20_000), 50, 1, &[999], 0).full);
        assert!(operate(&mut joules(49), 50, 1, &[999], 10).short_energy);
    }

    #[test]
    fn its_tanks_take_and_give_by_the_mode() {
        // Condensentrating: the chemical comes in from outside, the fluid goes out.
        assert!(chemical_in(false, Automation::External) && !chemical_out(false, Automation::External));
        assert!(fluid_out(false, Automation::External) && !fluid_in(false, Automation::External));
        // Decondensentrating: the other way; the machine itself always fills its output, a player always empties.
        assert!(!chemical_in(true, Automation::External) && chemical_out(true, Automation::External));
        assert!(chemical_in(true, Automation::Internal) && chemical_out(false, Automation::Manual));
        assert!(fluid_in(true, Automation::External) && !fluid_out(true, Automation::External));
    }
}
