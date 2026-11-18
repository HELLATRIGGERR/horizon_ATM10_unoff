use super::*;

impl Game<'_> {
    /// `IFluidHandler.fill` on the block entity at `pos`, a mod's: `resource` (never empty) offered
    /// through `face`, as in `fluid_handler_tanks`. Set `filled` (0 to the resource's amount) when
    /// `entity.id` is yours and `filled` is none yet; with `simulate` (`FluidAction.SIMULATE`),
    /// answer what you would take and change nothing. The entity is out of the world during the
    /// call, as in `fe_receive`; the engine puts it back and, when it took something for real,
    /// marks it changed (`setChanged`). The same block, side and operation asked again inside its
    /// own call gets 0, and so does a chain 16 calls deep.
    pub(in crate::game) fn native_fluid_handler_fill(
        &mut self,
        call: mod_api::__private::FluidHandlerFillCall<'_>,
    ) {
        self.fire_fluid_handler_fill(call);
    }
}
