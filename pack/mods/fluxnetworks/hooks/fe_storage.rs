// The Forge Energy of a plug or a point (`IEnergyStorage` readback).
fn flux_fe_storage(
    dimension: &str,
    pos: [i32; 3],
    entity: &mod_api::ModBlockEntity,
    info: &mut Option<mod_api::NativeFeInfo>,
) {
    crate::game::flux_world::fe_storage(dimension, pos, entity, info);
}
