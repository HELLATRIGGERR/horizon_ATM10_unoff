// A Furnator's Forge Energy on one side (`IEnergyStorage` readback).
fn powah_fe_storage(entity: &mod_api::ModBlockEntity, info: &mut Option<mod_api::NativeFeInfo>) {
    crate::game::powah_world::fe_storage(entity, info);
}
