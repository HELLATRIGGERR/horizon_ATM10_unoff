// A fluid asked of an apparatus' tank (`FluidTank.drain`; the hook passes one call).
fn theurgy_fluid_handler_drain(
    call: mod_api::__private::FluidHandlerDrainCall<'_>,
) -> mod_api::__private::FluidHandlerDrainCall<'_> {
    crate::game::theurgy_world::fluid_drain(&mut *call.entity, call.resource, call.max, call.simulate, &mut *call.drained);
    call
}
