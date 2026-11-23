// A fluid offered to an apparatus' tank (`FluidTank.fill`; the hook passes one call).
fn theurgy_fluid_handler_fill(
    call: mod_api::__private::FluidHandlerFillCall<'_>,
) -> mod_api::__private::FluidHandlerFillCall<'_> {
    crate::game::theurgy_world::fluid_fill(&mut *call.entity, call.resource, call.simulate, &mut *call.filled);
    call
}
