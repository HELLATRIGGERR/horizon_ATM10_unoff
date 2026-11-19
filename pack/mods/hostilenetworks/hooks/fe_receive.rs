// Forge Energy offered to a machine (`IEnergyStorage.receiveEnergy`).
fn hostilenetworks_fe_receive(
    entity: &mut horizon_client::containers::ModBlockEntity,
    max: i64,
    simulate: bool,
    accepted: &mut Option<i64>,
) {
    crate::game::hostilenetworks_world::fe_receive(entity, max, simulate, accepted);
}
