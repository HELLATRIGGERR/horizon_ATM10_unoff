// A Furnator's Forge Energy on one side (`IEnergyStorage` readback).
fn powah_fe_storage(
    entity: &horizon_client::containers::ModBlockEntity,
    info: &mut Option<crate::game::native_hooks::NativeFeInfo>,
) {
    crate::game::powah_world::fe_storage(entity, info);
}
