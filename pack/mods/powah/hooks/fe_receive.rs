// Forge Energy offered to a Furnator: it refuses it (`IEnergyStorage.receiveEnergy`).
fn powah_fe_receive(entity: &horizon_client::containers::ModBlockEntity, accepted: &mut Option<i64>) {
    crate::game::powah_world::fe_receive(entity, accepted);
}
