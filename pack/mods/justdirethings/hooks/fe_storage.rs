// The generator's Forge Energy on one side (`IEnergyStorage` readback): it gives and never takes.
fn justdirethings_fe_storage(entity: &mod_api::ModBlockEntity, info: &mut Option<mod_api::NativeFeInfo>) {
    crate::game::justdirethings_world::fe_storage(entity, info);
}
