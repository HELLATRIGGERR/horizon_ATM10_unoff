// Forge Energy offered to a machine (`IEnergyStorage.receiveEnergy`; the hook passes one call).
fn hostilenetworks_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    crate::game::hostilenetworks_world::fe_receive(&mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    call
}
