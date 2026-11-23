// Fluid offered to the Paradox Machine or the Fluid Collector T1 (`IFluidHandler.fill`; the hook passes one call).
fn justdirethings_fluid_handler_fill(call: mod_api::__private::FluidHandlerFillCall<'_>) -> mod_api::__private::FluidHandlerFillCall<'_> {
    if call.filled.is_none() {
        if call.entity.id() == crate::game::justdirethings_time::PARADOX {
            crate::game::justdirethings_time::fluid_fill(&mut *call.entity, call.resource, call.simulate, &mut *call.filled);
        } else if call.entity.id() == crate::game::justdirethings_machines::COLLECTOR {
            crate::game::justdirethings_machines::fluid_fill(&mut *call.entity, call.resource, call.simulate, &mut *call.filled);
        }
    }
    call
}
