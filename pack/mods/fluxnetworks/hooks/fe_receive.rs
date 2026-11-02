// `receiveEnergy` on a plug or a point.
fn flux_fe_receive(
    dimension: &str,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
    max: i64,
    simulate: bool,
    accepted: &mut Option<i64>,
) {
    crate::game::flux_world::fe_receive(dimension, pos, entity, max, simulate, accepted);
}
