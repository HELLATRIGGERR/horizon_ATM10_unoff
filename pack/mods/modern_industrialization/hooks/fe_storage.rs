// An MI machine's energy as Forge Energy (`EnergyApi`'s `WrappedMIStorage`).
fn mi_fe_storage(entity: &mod_api::ModBlockEntity, info: &mut Option<mod_api::NativeFeInfo>) {
    crate::game::mi_world::fe_storage(entity, info);
}
