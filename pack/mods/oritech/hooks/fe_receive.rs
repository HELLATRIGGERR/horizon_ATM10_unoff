// Forge Energy offered to a machine (`IEnergyStorage.receiveEnergy`; the hook passes one call).
fn oritech_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    crate::game::oritech_world::fe_receive(&mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    call
}
