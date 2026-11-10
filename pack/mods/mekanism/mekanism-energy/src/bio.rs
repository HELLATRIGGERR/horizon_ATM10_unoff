//! Mekanism Generators' Bio-Generator (`TileEntityBioGenerator`): a
//! bioethanol tank (`[bio_generator] tankCapacity`, 24 000 mB on ATM10) that
//! any block fills on every side but the front (`VariableCapacityFluidTank.input`
//! with the `c:bioethanol` validator) and none drains, a fuel slot
//! (`FluidFuelInventorySlot.forFuel`: a container of bioethanol empties into
//! the tank; an item of `c:fuels/bio` gives 200 mB, of `c:fuels/block/bio`
//! 1800), and a burn of one mB a tick for `bioGeneration` Joules (350 on
//! ATM10) while all of them fit in its store, out of its front at twice that.
use crate::config::GeneratorsConfig;
use crate::container::{Automation, EnergyContainer};
use crate::heat::{self, Fuel, SlotUse};

/// `Items.FUELS_BIO`'s value, in mB.
pub const BIO_FUEL: i32 = 200;
/// `Items.FUELS_BLOCK_BIO`'s value, in mB.
pub const BLOCK_FUEL: i32 = 1800;
/// The item tags the fuel slot reads (`MekanismTags.Items.FUELS_BIO`, `FUELS_BLOCK_BIO`).
pub const BIO_TAG: &str = "c:fuels/bio";
pub const BLOCK_TAG: &str = "c:fuels/block/bio";
/// The fluid tag its tank takes (`GeneratorTags.Fluids.BIOETHANOL`).
pub const FLUID_TAG: &str = "c:bioethanol";
/// The fluid a burnt item turns into (`GeneratorsFluids.BIOETHANOL`).
pub const BIOETHANOL: &str = "mekanismgenerators:bioethanol";

/// The slot's `fuelValue`: 200 for `c:fuels/bio`, else 1800 for
/// `c:fuels/block/bio`, else none.
pub fn fuel_value(in_bio: bool, in_block: bool) -> i32 {
    if in_bio {
        BIO_FUEL
    } else if in_block {
        BLOCK_FUEL
    } else {
        0
    }
}

/// `FluidFuelInventorySlot.fillOrBurn` on the bioethanol tank: a container
/// alone that holds what the tank takes fills it, else an item worth `value`
/// mB goes in whole when it fits (`fuel.takes_lava` says whether the tank
/// takes bioethanol: it is empty or holds it).
pub fn fill_or_burn(tank: &mut u32, capacity: u32, fuel: Option<Fuel>, value: i32) -> SlotUse {
    heat::fill_or_burn_value(tank, capacity, fuel, value)
}

/// `TileEntityGenerator`'s max output: twice `bioGeneration`.
pub fn max_output(config: &GeneratorsConfig) -> u64 {
    config.bio_generation.saturating_mul(2)
}

/// The rest of `onUpdateServer` after the ejector and the slots: with
/// bioethanol in the tank and room for a whole tick's Joules, one mB burns
/// into `bioGeneration` Joules (`INTERNAL`). Whether it burnt (`setActive`).
pub fn burn(tank: &mut u32, energy: &mut EnergyContainer, generation: u64, can_function: bool) -> bool {
    if !can_function || *tank == 0 || energy.insert(generation, true, Automation::Internal) != 0 {
        return false;
    }
    *tank -= 1;
    energy.insert(generation, false, Automation::Internal);
    true
}

/// `getProductionRate`: `bioGeneration` while active.
pub fn production(active: bool, config: &GeneratorsConfig) -> u64 {
    if active { config.bio_generation } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atm10() -> GeneratorsConfig {
        GeneratorsConfig { bio_generation: 350, bio_storage: 160_000, ..GeneratorsConfig::default() }
    }

    fn item(count: u32) -> Fuel {
        Fuel { count, fluid: None, burn_time: 0, has_remainder: false, takes_lava: true }
    }

    #[test]
    fn a_mb_a_tick_for_350_joules() {
        // Measured on the oracle (g2/w1, B1 and B2): 350 J and one mB a tick; a store
        // without room for a whole 350 J burns nothing.
        let config = atm10();
        let mut store = EnergyContainer::output(config.bio_storage);
        let mut tank = 1000;
        let rows: Vec<(u64, u32)> = (0..5)
            .map(|_| {
                assert!(burn(&mut tank, &mut store, config.bio_generation, true));
                (store.stored, tank)
            })
            .collect();
        assert_eq!(rows, [(350, 999), (700, 998), (1050, 997), (1400, 996), (1750, 995)]);
        store.stored = 159_700;
        assert!(!burn(&mut tank, &mut store, 350, true));
        assert_eq!((store.stored, tank), (159_700, 995));
        let mut empty = 0;
        assert!(!burn(&mut empty, &mut EnergyContainer::output(1000), 350, true));
        assert_eq!((max_output(&config), production(true, &config), production(false, &config)), (700, 350, 0));
    }

    #[test]
    fn bio_fuel_fills_the_tank_before_it_burns() {
        // B3 and B3b: two bio fuel, 200 mB each, one a tick; a block of it 1800 mB.
        let config = atm10();
        let mut store = EnergyContainer::output(config.bio_storage);
        let mut tank = 0;
        assert_eq!(fill_or_burn(&mut tank, 24_000, Some(item(2)), fuel_value(true, false)), SlotUse::ShrunkOne);
        burn(&mut tank, &mut store, 350, true);
        assert_eq!(tank, 199);
        assert_eq!(fill_or_burn(&mut tank, 24_000, Some(item(1)), fuel_value(true, false)), SlotUse::ShrunkOne);
        burn(&mut tank, &mut store, 350, true);
        assert_eq!(tank, 398);
        let mut block = 0;
        assert_eq!(fill_or_burn(&mut block, 24_000, Some(item(1)), fuel_value(false, true)), SlotUse::ShrunkOne);
        assert_eq!(block, 1800);
        let mut nearly = 23_900;
        assert_eq!(fill_or_burn(&mut nearly, 24_000, Some(item(1)), BIO_FUEL), SlotUse::Kept, "200 mB don't fit");
        assert_eq!(fuel_value(false, false), 0);
        assert_eq!(fill_or_burn(&mut tank, 24_000, Some(item(1)), 0), SlotUse::Kept, "not a fuel");
    }
}
