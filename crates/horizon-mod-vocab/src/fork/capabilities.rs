//! The fork's block capabilities as NeoForge 1.21.1 has them: `IEnergyStorage`, `IFluidHandler`
//! and Mekanism's `IChemicalHandler`, read through the `fe_storage`, `fluid_handler_tanks` and
//! `chemical_handler_tanks` hooks.
use crate::FluidStack;

/// A block's Forge Energy through one side, as NeoForge 1.21.1's `IEnergyStorage`
/// reads it (`fe_storage`, `native_fe_info`), in FE.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeFeInfo {
    /// `getEnergyStored`.
    pub stored: i64,
    /// `getMaxEnergyStored`.
    pub capacity: i64,
    /// `canReceive`: whether a receive may take anything.
    pub can_receive: bool,
    /// `canExtract`: whether an extract may give anything.
    pub can_extract: bool,
}

/// One tank of a block's fluid handler through one side, as NeoForge's `IFluidHandler` reads it
/// (`getFluidInTank`, `getTankCapacity`; `fluid_handler_tanks`, `native_fluid_handler`), in
/// millibuckets.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeFluidTankInfo {
    /// `getFluidInTank`: what it holds, none when nothing.
    pub stack: Option<FluidStack>,
    /// `getTankCapacity`.
    pub capacity: i32,
    /// Whether a simulated `drain` of `stack` through this side gives any of it: what AE2's
    /// storage bus asks of each tank to show only what it can take (`ExternalStorageFacade`'s
    /// extractable-only filter). False for an empty tank.
    pub drainable: bool,
}

impl Default for NativeFluidTankInfo {
    /// An empty tank that holds nothing.
    fn default() -> Self {
        Self {
            stack: None,
            capacity: 0,
            drainable: false,
        }
    }
}

/// One tank of a block's chemical handler through one side, as Mekanism's `IChemicalHandler`
/// reads it (`getChemicalInTank`, `getChemicalTankCapacity`; `chemical_handler_tanks`), in
/// millibuckets. The engine knows no chemical: one is its registry id (`mekanism:hydrogen`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeChemicalTankInfo {
    /// The chemical it holds, empty when nothing.
    pub chemical: String,
    /// How much of it.
    pub amount: i64,
    /// `getChemicalTankCapacity`.
    pub capacity: i64,
    /// Whether a simulated extraction of `chemical` through this side gives any of it: what
    /// Applied Mekanistics' storage bus asks of each tank to show only what it can take
    /// (`ChemicalHandlerFacade`'s extractable-only filter). False for an empty tank.
    pub extractable: bool,
}
