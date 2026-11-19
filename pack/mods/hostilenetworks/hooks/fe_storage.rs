// A machine's Forge Energy on one side (`IEnergyStorage` readback): it takes and never gives.
fn hostilenetworks_fe_storage(entity: &mod_api::ModBlockEntity, info: &mut Option<mod_api::NativeFeInfo>) {
    crate::game::hostilenetworks_world::fe_storage(entity, info);
}
