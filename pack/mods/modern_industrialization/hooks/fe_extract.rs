// An MI machine gives no Forge Energy (the hook passes one call).
fn mi_fe_extract(call: mod_api::__private::FeExtractCall<'_>) -> mod_api::__private::FeExtractCall<'_> {
    crate::game::mi_world::fe_extract(&*call.entity, &mut *call.given);
    call
}
