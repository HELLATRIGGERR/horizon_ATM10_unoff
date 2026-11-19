// An MI machine's energy as Forge Energy (`EnergyApi`'s `WrappedMIStorage`).
fn mi_fe_storage(
    entity: &horizon_client::containers::ModBlockEntity,
    info: &mut Option<crate::game::native_hooks::NativeFeInfo>,
) {
    crate::game::mi_world::fe_storage(entity, info);
}
