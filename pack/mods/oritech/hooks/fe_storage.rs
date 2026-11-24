// A machine's Forge Energy on any side (`IEnergyStorage` readback): it takes and never gives.
fn oritech_fe_storage(entity: &mod_api::ModBlockEntity, info: &mut Option<mod_api::NativeFeInfo>) {
    crate::game::oritech_world::fe_storage(entity, info);
}
