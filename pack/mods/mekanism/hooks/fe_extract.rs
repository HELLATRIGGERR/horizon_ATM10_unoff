// Forge Energy drawn from a Mekanism cube or cable (`IEnergyStorage.extractEnergy`).
fn mek_fe_extract(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mut horizon_client::containers::ModBlockEntity,
    max: i64,
    simulate: bool,
    given: &mut Option<i64>,
) {
    crate::game::mek_world::fe_transfer(dimension, pos, face, entity, max, simulate, true, given);
}
