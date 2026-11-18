use super::*;

impl Game<'_> {
    /// `IChemicalHandler.extractChemical(stack, action)` on the block at `pos`, as
    /// `chemical_handler_insert`: at most `amount` of `chemical` and only that one. Set `extracted`
    /// (0 to `amount`) to what leaves.
    pub(in crate::game) fn native_chemical_handler_extract(
        &mut self,
        call: mod_api::__private::ChemicalHandlerExtractCall<'_>,
    ) {
        self.fire_chemical_handler_extract(call);
    }
}
