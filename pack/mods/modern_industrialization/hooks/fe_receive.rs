// Forge Energy offered to an MI machine, taken in whole EU (the hook passes one call).
fn mi_fe_receive(call: mod_api::__private::FeReceiveCall<'_>) -> mod_api::__private::FeReceiveCall<'_> {
    crate::game::mi_world::fe_receive(&mut *call.entity, call.max, call.simulate, &mut *call.accepted);
    call
}
