//! Mekanism's Electrolytic Separator (`TileEntityElectrolyticSeparator`): a
//! fluid in (24 000 mB, `mekanism:separating` recipes: 2 mB of water make
//! 2 mB of hydrogen on the left and 1 mB of oxygen on the right) and two
//! chemicals out into tanks of 2400 mB, `2^speed` operations a tick at twice
//! hydrogen's energy density (its fuel data map: 200 J/mB on ATM10) times the
//! recipe's multiplier, each operation's Joules (`FixedUsageEnergyContainer`:
//! energy upgrades don't lower it, they grow the store). Each output tank has
//! a dump mode (`GasMode`): idle, dumping excess (down to `dumpExcessKeepRatio`
//! of its capacity, at most `chemicalAutoEjectRate` a tick) or dumping
//! (`8 * 2^speed` mB a tick). With both tanks dumping excess and both at that
//! level it stops (`canFunction`). The numbers were measured on a NeoForge
//! 1.21.1 server with Mekanism 10.7.19 and ATM10's configs.
use crate::chemical::{ChemicalStack, ChemicalTank, TankRules, always, any, internal_only};
use crate::container::{Automation, EnergyContainer};

/// `TileEntityElectrolyticSeparator.MAX_GAS`: each output tank, in mB.
pub const MAX_GAS: i64 = 2400;
/// `MAX_FLUID`: the input tank, in mB.
pub const MAX_FLUID: i32 = 24_000;
/// `BASE_DUMP_RATE`, mB a tick.
pub const BASE_DUMP_RATE: i64 = 8;

/// `TileEntityChemicalTank.GasMode`, saved by ordinal (`dump_left`, `dump_right`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GasMode {
    #[default]
    Idle,
    DumpingExcess,
    Dumping,
}

impl GasMode {
    pub const ALL: [Self; 3] = [Self::Idle, Self::DumpingExcess, Self::Dumping];

    pub fn id(self) -> i32 {
        self as i32
    }

    /// `BY_ID` (`ByIdMap.continuous`, wrapping).
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(3) as usize]
    }

    /// `getNext`, the button's left click.
    pub fn next(self) -> Self {
        Self::by_id(self.id() + 1)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::DumpingExcess => "dumping_excess",
            Self::Dumping => "dumping",
        }
    }
}

/// The output tanks' rules (`BasicChemicalTank.output`): anything takes out,
/// only the separator puts in, every chemical valid (`ALWAYS_ALLOW`).
pub fn output_rules() -> TankRules<'static> {
    TankRules { can_extract: &always, can_insert: &internal_only, valid: &any }
}

/// `recalculateUpgrades(SPEED)`: operations a tick and the dump rate.
pub fn speed(speed_upgrades: u32) -> (u32, i64) {
    let factor = 2f64.powi(speed_upgrades as i32);
    (factor as i32 as u32, (BASE_DUMP_RATE as f64 * factor) as i64)
}

/// Joules an operation: `2 * hydrogenEnergyDensity` (`MathUtils.multiplyClamped`)
/// times the recipe's `energy_multiplier`.
pub fn energy_per_tick(hydrogen_density: u64, multiplier: u64) -> u64 {
    hydrogen_density.saturating_mul(2).saturating_mul(multiplier)
}

/// `getDumpingExcessTarget`: `clampToLong(capacity * ratio)`.
pub fn excess_target(capacity: i64, ratio: f64) -> i64 {
    crate::clamp_to_long(capacity as f64 * ratio)
}

/// `canFunction`'s own part: not when both tanks dump excess and both are at
/// their target.
pub fn can_function(left: (GasMode, &ChemicalTank), right: (GasMode, &ChemicalTank), ratio: f64) -> bool {
    let at = |tank: &ChemicalTank| tank.stored() >= excess_target(tank.capacity, ratio);
    !(left.0 == GasMode::DumpingExcess && right.0 == GasMode::DumpingExcess && at(left.1) && at(right.1))
}

/// `handleTank`: a dumping tank loses `dump_rate` a tick, one dumping excess
/// what it holds over its target, at most `eject_rate` (`chemicalAutoEjectRate`).
pub fn handle_tank(tank: &mut ChemicalTank, mode: GasMode, dump_rate: i64, eject_rate: i64, ratio: f64) {
    if tank.is_empty() {
        return;
    }
    match mode {
        GasMode::Dumping => {
            tank.shrink_stack(dump_rate, false);
        }
        GasMode::DumpingExcess => {
            let target = excess_target(tank.capacity, ratio);
            let stored = tank.stored();
            if target < stored {
                tank.shrink_stack((stored - target).min(eject_rate), false);
            }
        }
        GasMode::Idle => {}
    }
}

/// A recipe's outputs per operation and what its input takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Separation {
    /// mB of the fluid an operation takes.
    pub input: i64,
    pub left: ChemicalStack,
    pub right: ChemicalStack,
}

/// `OutputHelper.calculateOperationsCanSupport` for a chemical tank: how many
/// operations' `out` fit (an internal insert of all it could).
pub fn room_for(tank: &ChemicalTank, out: &ChemicalStack) -> u32 {
    if out.is_empty() {
        return u32::MAX;
    }
    let mut probe = tank.clone();
    let max = out.copy_with_amount(i64::MAX);
    let rules = output_rules();
    let used = i64::MAX - probe.insert(&max, true, Automation::Internal, &rules).amount();
    (used / out.amount()).clamp(0, i64::from(i32::MAX)) as u32
}

/// What one tick of the cached recipe did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    pub operations: u32,
    /// Joules used.
    pub energy: u64,
    /// `NOT_ENOUGH_ENERGY`, `NOT_ENOUGH_INPUT`, no room on the left or right.
    pub short_energy: bool,
    pub short_input: bool,
    pub left_full: bool,
    pub right_full: bool,
}

/// One tick of `OneInputCachedRecipe.separating` (`CachedRecipe.process`):
/// `baseline` operations, fewer for the Joules stored (`maxForEnergy`), the
/// input held and the room in each output tank; then the energy, the input
/// and both outputs at once. `held` is the input tank's amount of the
/// recipe's fluid.
#[allow(clippy::too_many_arguments)]
pub fn process(
    recipe: &Separation,
    held: i64,
    left: &mut ChemicalTank,
    right: &mut ChemicalTank,
    energy: &mut EnergyContainer,
    per_tick: u64,
    baseline: u32,
    can_function: bool,
) -> (Ran, i64) {
    let mut ran = Ran::default();
    if !can_function {
        return (ran, 0);
    }
    let mut operations = baseline;
    if per_tick != 0 {
        let for_energy = (energy.stored / per_tick).min(i32::MAX as u64) as u32;
        if for_energy == 0 {
            ran.short_energy = true;
            return (ran, 0);
        }
        operations = operations.min(for_energy);
    }
    let by_input = if recipe.input > 0 { (held / recipe.input).clamp(0, i64::from(i32::MAX)) as u32 } else { u32::MAX };
    if by_input == 0 {
        ran.short_input = true;
        return (ran, 0);
    }
    operations = operations.min(by_input);
    let (room_left, room_right) = (room_for(left, &recipe.left), room_for(right, &recipe.right));
    ran.left_full = room_left == 0;
    ran.right_full = room_right == 0;
    operations = operations.min(room_left).min(room_right);
    if operations == 0 {
        return (ran, 0);
    }
    let n = i64::from(operations);
    ran.energy = energy.extract(per_tick.saturating_mul(u64::from(operations)), false, Automation::Internal);
    let rules = output_rules();
    left.insert(&recipe.left.copy_with_amount(recipe.left.amount() * n), false, Automation::Internal, &rules);
    right.insert(&recipe.right.copy_with_amount(recipe.right.amount() * n), false, Automation::Internal, &rules);
    ran.operations = operations;
    (ran, recipe.input * n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water() -> Separation {
        Separation { input: 2, left: ChemicalStack::new("mekanism:hydrogen", 2), right: ChemicalStack::new("mekanism:oxygen", 1) }
    }

    fn store(joules: u64) -> EnergyContainer {
        let mut c = EnergyContainer::basic(160_000);
        c.stored = joules;
        c
    }

    /// One tick as the separator runs it: the recipe, then both dumps.
    #[allow(clippy::too_many_arguments)]
    fn tick(water_mb: &mut i64, left: &mut ChemicalTank, right: &mut ChemicalTank, energy: &mut EnergyContainer, upgrades: u32, modes: (GasMode, GasMode)) -> u32 {
        let (baseline, dump) = speed(upgrades);
        let can = can_function((modes.0, left), (modes.1, right), 0.85);
        let (ran, used) = process(&water(), *water_mb, left, right, energy, energy_per_tick(200, 1), baseline, can);
        *water_mb -= used;
        handle_tank(left, modes.0, dump, 1024, 0.85);
        handle_tank(right, modes.1, dump, 1024, 0.85);
        ran.operations
    }

    fn tanks() -> (ChemicalTank, ChemicalTank) {
        (ChemicalTank::new(MAX_GAS), ChemicalTank::new(MAX_GAS))
    }

    #[test]
    fn water_makes_hydrogen_and_oxygen_at_400_joules_an_operation() {
        // Measured: 2 mB of water, 2 H2, 1 O2 and 400 J a tick.
        let (mut l, mut r) = tanks();
        let (mut w, mut e) = (24_000, store(160_000));
        for t in 1..=6 {
            assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, (GasMode::Idle, GasMode::Idle)), 1);
            assert_eq!((w, l.stored(), r.stored(), e.stored), (24_000 - 2 * t, 2 * t, t, 160_000 - 400 * t as u64));
        }
        assert_eq!(l.stored.id(), Some("mekanism:hydrogen"));
    }

    #[test]
    fn energy_limits_the_operations() {
        let (mut l, mut r) = tanks();
        let (mut w, mut e) = (24_000, store(399));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 0, "399 J: nothing");
        e.stored = 800;
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 1);
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 1);
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 0);
        assert_eq!((w, l.stored(), r.stored(), e.stored), (23_996, 4, 2, 0));
        // 3 speed upgrades: 8 operations, 3200 J; 8 of them 256, then what the rest pays for.
        let (mut l, mut r) = tanks();
        let (mut w, mut e) = (24_000, store(160_000));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 3, Default::default()), 8);
        assert_eq!((w, l.stored(), r.stored(), e.stored), (23_984, 16, 8, 156_800));
        let (mut l, mut r) = tanks();
        let (mut w, mut e) = (24_000, store(160_000));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 8, Default::default()), 256);
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 8, Default::default()), 144);
        assert_eq!((w, l.stored(), r.stored(), e.stored), (23_200, 800, 400, 0));
    }

    #[test]
    fn a_full_or_foreign_output_stops_it() {
        let (mut w, mut e) = (24_000, store(160_000));
        let (mut l, mut r) = (ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", 2399)), ChemicalTank::new(MAX_GAS));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 0, "an operation needs 2 mB of room");
        let (mut l, mut r) = (ChemicalTank::new(MAX_GAS), ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:oxygen", 2399)));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 1);
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 0);
        let (mut l, mut r) = (ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:oxygen", 10)), ChemicalTank::new(MAX_GAS));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, Default::default()), 0, "oxygen on the left: no room for hydrogen");
    }

    #[test]
    fn dump_modes_as_measured() {
        let (mut w, mut e) = (0, store(160_000));
        let (mut l, mut r) = (ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", 100)), ChemicalTank::new(MAX_GAS));
        for left in [92, 84, 76] {
            tick(&mut w, &mut l, &mut r, &mut e, 0, (GasMode::Dumping, GasMode::Idle));
            assert_eq!(l.stored(), left);
        }
        l = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", 100));
        tick(&mut w, &mut l, &mut r, &mut e, 3, (GasMode::Dumping, GasMode::Idle));
        assert_eq!(l.stored(), 36, "64 a tick with 3 speed upgrades");
        tick(&mut w, &mut l, &mut r, &mut e, 3, (GasMode::Dumping, GasMode::Idle));
        assert!(l.is_empty());
        for (from, to) in [(2400, 2040), (2100, 2040), (2041, 2040), (2040, 2040), (100, 100)] {
            l = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", from));
            tick(&mut w, &mut l, &mut r, &mut e, 0, (GasMode::DumpingExcess, GasMode::Idle));
            assert_eq!(l.stored(), to, "excess from {from}");
        }
        // Both dumping excess and both at 2040: it stops; one below: it runs.
        w = 24_000;
        let both = (GasMode::DumpingExcess, GasMode::DumpingExcess);
        let mut l = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", 2040));
        let mut r = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:oxygen", 2040));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, both), 0);
        let mut l = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", 2039));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, both), 1);
        assert_eq!((l.stored(), r.stored()), (2040, 2040), "2041 dumped back to 2040, oxygen 2041 too");
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, both), 0);
        // Left full and dumping excess, right low: no operation that tick, the dump after it.
        let mut l = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:hydrogen", 2400));
        let mut r = ChemicalTank::with(MAX_GAS, ChemicalStack::new("mekanism:oxygen", 100));
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, both), 0);
        assert_eq!(l.stored(), 2040);
        assert_eq!(tick(&mut w, &mut l, &mut r, &mut e, 0, both), 1);
        assert_eq!(r.stored(), 101);
        assert_eq!(GasMode::by_id(3), GasMode::Idle);
        assert_eq!(GasMode::Dumping.next(), GasMode::Idle);
    }
}
