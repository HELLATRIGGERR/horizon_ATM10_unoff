// Fluid asked of the Paradox Machine or the Fluid Collector T1 (`IFluidHandler.drain`; the hook passes one call).
fn justdirethings_fluid_handler_drain(call: mod_api::__private::FluidHandlerDrainCall<'_>) -> mod_api::__private::FluidHandlerDrainCall<'_> {
    if call.drained.is_none() {
        if call.entity.id() == crate::game::justdirethings_time::PARADOX {
            crate::game::justdirethings_time::fluid_drain(&mut *call.entity, call.resource, call.max, call.simulate, &mut *call.drained);
        } else if call.entity.id() == crate::game::justdirethings_machines::COLLECTOR
            || call.entity.id() == crate::game::justdirethings_more::FLUID_PLACER
            || call.entity.id() == crate::game::justdirethings_more::FLUID_GEN
            || crate::game::justdirethings_t2x::is_fluid(call.entity.id())
        {
            crate::game::justdirethings_machines::fluid_drain(&mut *call.entity, call.resource, call.max, call.simulate, &mut *call.drained);
        }
    }
    call
}
