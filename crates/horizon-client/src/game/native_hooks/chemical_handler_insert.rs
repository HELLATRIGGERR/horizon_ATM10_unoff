use super::*;

impl Game<'_> {
    /// `IChemicalHandler.insertChemical` on the block at `pos`, as `chemical_handler_tanks`:
    /// `amount` (1 or more) of `chemical` offered through `face`. Set `inserted` (0 to `amount`)
    /// when the block is yours and `inserted` is none yet; with `simulate`, answer what you would
    /// take and change nothing.
    pub(in crate::game) fn native_chemical_handler_insert(
        &mut self,
        call: mod_api::__private::ChemicalHandlerInsertCall<'_>,
    ) {
        self.fire_chemical_handler_insert(call);
    }
}
