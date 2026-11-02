// The Forge Energy of a Mekanism cube or cable on one side (`IEnergyStorage` readback).
fn mek_fe_storage(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &horizon_client::containers::ModBlockEntity,
    info: &mut Option<crate::game::native_hooks::NativeFeInfo>,
) {
    crate::game::mek_world::fe_storage(dimension, pos, face, entity, info);
}
