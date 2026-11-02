// The Forge Energy of a plug or a point (`IEnergyStorage` readback).
fn flux_fe_storage(
    dimension: &str,
    pos: [i32; 3],
    entity: &horizon_client::containers::ModBlockEntity,
    info: &mut Option<crate::game::native_hooks::NativeFeInfo>,
) {
    crate::game::flux_world::fe_storage(dimension, pos, entity, info);
}
