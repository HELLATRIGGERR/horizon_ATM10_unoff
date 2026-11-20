// A fluid offered to a Mekanism block (`IFluidHandler.fill`; the hook passes one call).
fn mek_fluid_handler_fill(
    call: mod_api::__private::FluidHandlerFillCall<'_>,
) -> mod_api::__private::FluidHandlerFillCall<'_> {
    crate::game::mek_world::fluid_fill(call.dimension, call.pos, call.face, &mut *call.entity, call.resource, call.simulate, &mut *call.filled);
    call
}
