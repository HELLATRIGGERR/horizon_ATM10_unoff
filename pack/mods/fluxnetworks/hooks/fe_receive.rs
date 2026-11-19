// `receiveEnergy` on a plug or a point (the hook passes one call).
fn flux_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    crate::game::flux_world::fe_receive(call.dimension, call.pos, &mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    call
}
