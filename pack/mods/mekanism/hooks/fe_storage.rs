// The Forge Energy of a Mekanism cube or cable on one side (`IEnergyStorage` readback).
fn mek_fe_storage(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mod_api::ModBlockEntity,
    info: &mut Option<mod_api::NativeFeInfo>,
) {
    crate::game::mek_world::api_fe_storage(dimension, pos, face, entity, info);
}
