// A machine's Forge Energy on one side (`IEnergyStorage` readback): it takes and never gives.
fn hostilenetworks_fe_storage(
    entity: &horizon_client::containers::ModBlockEntity,
    info: &mut Option<crate::game::native_hooks::NativeFeInfo>,
) {
    crate::game::hostilenetworks_world::fe_storage(entity, info);
}
