// Forge Energy drawn from a Mekanism cube or cable (`IEnergyStorage.extractEnergy`; the hook passes one call).
fn mek_fe_extract(call: mod_api::__private::FeExtractCall<'_>) -> mod_api::__private::FeExtractCall<'_> {
    crate::game::mek_world::api_fe_transfer(call.dimension, call.pos, call.face, &mut *call.entity, call.max, call.simulate, true, &mut *call.given);
    call
}
