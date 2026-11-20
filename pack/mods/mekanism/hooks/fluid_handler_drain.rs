// A fluid asked of a Mekanism block (`IFluidHandler.drain`; the hook passes one call).
fn mek_fluid_handler_drain(
    call: mod_api::__private::FluidHandlerDrainCall<'_>,
) -> mod_api::__private::FluidHandlerDrainCall<'_> {
    crate::game::mek_world::fluid_drain(call.dimension, call.pos, call.face, &mut *call.entity, call.resource, call.max, call.simulate, &mut *call.drained);
    call
}
