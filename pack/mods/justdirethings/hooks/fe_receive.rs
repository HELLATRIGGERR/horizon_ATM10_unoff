// Forge Energy offered to the Paradox Machine (`IEnergyStorage.receiveEnergy`; the hook passes one call).
fn justdirethings_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    if call.entity.id() == crate::game::justdirethings_time::PARADOX && call.accepted.is_none() {
        crate::game::justdirethings_time::fe_receive(&mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    }
    // The T2 machines' buffers (`PoweredMachineBE`).
    let id = call.entity.id();
    if (id == crate::game::justdirethings_clicker::CLICKER_T2 || crate::game::justdirethings_t2::owns(id)) && call.accepted.is_none() {
        crate::game::justdirethings_clicker::fe_receive(&mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    }
    call
}
