use super::*;

impl Game<'_> {
    /// `IFluidHandler.drain` on the block entity at `pos`, a mod's, as `fluid_handler_fill`: with a
    /// `resource`, `drain(FluidStack)` (at most its amount, and only that fluid with those
    /// components), without one, `drain(int)` of at most `max` (1 or more) of whatever comes out
    /// first. Set `drained` to what leaves (the empty stack for nothing).
    pub(in crate::game) fn native_fluid_handler_drain(
        &mut self,
        call: mod_api::__private::FluidHandlerDrainCall<'_>,
    ) {
        self.fire_fluid_handler_drain(call);
    }
}
