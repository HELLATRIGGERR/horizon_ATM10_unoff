// A fluid offered to an Oritech machine (`IFluidHandler.fill`; the hook passes one call).
fn oritech_fluid_handler_fill(call: mod_api::__private::FluidHandlerFillCall<'_>) -> mod_api::__private::FluidHandlerFillCall<'_> {
    crate::game::oritech_world::fluid_fill(&mut *call.entity, call.resource, call.simulate, &mut *call.filled);
    call
}
