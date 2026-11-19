// Forge Energy offered to a Furnator: it refuses it (`IEnergyStorage.receiveEnergy`; the hook passes one call).
fn powah_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    crate::game::powah_world::fe_receive(&*call.entity, &mut *call.accepted);
    call
}
