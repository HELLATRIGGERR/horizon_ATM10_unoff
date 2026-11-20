// Forge Energy offered to a Mekanism cube or cable (`IEnergyStorage.receiveEnergy`; the hook passes one call).
fn mek_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    crate::game::mek_world::api_fe_transfer(call.dimension, call.pos, call.face, &mut *call.entity, call.max, call.simulate, false, &mut *call.accepted);
    call
}
