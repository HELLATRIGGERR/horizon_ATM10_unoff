// Forge Energy offered to a Mekanism cube or cable (`IEnergyStorage.receiveEnergy`).
fn mek_fe_receive(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mut horizon_client::containers::ModBlockEntity,
    max: i64,
    simulate: bool,
    accepted: &mut Option<i64>,
) {
    crate::game::mek_world::fe_transfer(dimension, pos, face, entity, max, simulate, false, accepted);
}
