// A fluid asked of an Oritech machine (`IFluidHandler.drain`; the hook passes one call).
fn oritech_fluid_handler_drain(call: mod_api::__private::FluidHandlerDrainCall<'_>) -> mod_api::__private::FluidHandlerDrainCall<'_> {
    crate::game::oritech_world::fluid_drain(&mut *call.entity, call.resource, call.max, call.simulate, &mut *call.drained);
    call
}
