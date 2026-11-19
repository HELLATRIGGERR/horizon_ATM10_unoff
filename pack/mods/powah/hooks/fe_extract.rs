// Forge Energy drawn from a Furnator (`IEnergyStorage.extractEnergy`; the hook passes one call).
fn powah_fe_extract(call: mod_api::__private::FeExtractCall<'_>) -> mod_api::__private::FeExtractCall<'_> {
    crate::game::powah_world::fe_extract(&mut *call.entity, call.max, call.simulate, &mut *call.given);
    call
}
