// Forge Energy offered to the Paradox Machine (`IEnergyStorage.receiveEnergy`; the hook passes one call).
fn justdirethings_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    if call.entity.id() == crate::game::justdirethings_time::PARADOX && call.accepted.is_none() {
        crate::game::justdirethings_time::fe_receive(&mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    }
    call
}
