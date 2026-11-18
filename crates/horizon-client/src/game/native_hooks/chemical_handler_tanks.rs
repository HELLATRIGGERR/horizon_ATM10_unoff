use super::*;

impl Game<'_> {
    /// The chemical handler of the block at `pos` in `dimension`, a mod's, through `face`, as
    /// Mekanism's `IChemicalHandler` reads it (`getChemicalTanks`, `getChemicalInTank`,
    /// `getChemicalTankCapacity`; `Capabilities.CHEMICAL.block()`). The engine knows no chemical: a
    /// chemical is its registry id, an amount is in millibuckets. `face` is the side of this block
    /// the caller touches, as in `fluid_handler_tanks`; none is the block itself, a null side. Set
    /// `tanks` when the block is yours and `tanks` is none yet; leave it none on a side without a
    /// handler, so AE2's buses see nothing there. Asked on the server, often, and only to read:
    /// answer from your own state, change nothing.
    pub(in crate::game) fn native_chemical_handler_tanks(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        tanks: &mut Option<Vec<mod_api::NativeChemicalTankInfo>>,
    ) {
        self.fire_chemical_handler_tanks(dimension, pos, face, tanks);
    }
}
