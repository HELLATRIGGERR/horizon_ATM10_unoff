// Forge Energy drawn from a Furnator (`IEnergyStorage.extractEnergy`).
fn powah_fe_extract(
    entity: &mut horizon_client::containers::ModBlockEntity,
    max: i64,
    simulate: bool,
    given: &mut Option<i64>,
) {
    crate::game::powah_world::fe_extract(entity, max, simulate, given);
}
