// Forge Energy drawn from a generator (`IEnergyStorage.extractEnergy`; the hook passes one call).
fn oritech_fe_extract(call: mod_api::__private::FeExtractCall<'_>) -> mod_api::__private::FeExtractCall<'_> {
    crate::game::oritech_world::fe_extract(&mut *call.entity, call.max, call.simulate, &mut *call.given);
    call
}
