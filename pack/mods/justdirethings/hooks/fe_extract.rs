// Forge Energy drawn from a Generator T1 (`IEnergyStorage.extractEnergy`; the hook passes one call).
fn justdirethings_fe_extract(call: mod_api::__private::FeExtractCall<'_>) -> mod_api::__private::FeExtractCall<'_> {
    crate::game::justdirethings_world::fe_extract(&mut *call.entity, call.max, call.simulate, &mut *call.given);
    call
}
