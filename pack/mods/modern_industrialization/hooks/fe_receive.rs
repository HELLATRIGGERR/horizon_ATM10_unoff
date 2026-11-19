// Forge Energy offered to an MI machine, taken in whole EU.
fn mi_fe_receive(
    entity: &mut horizon_client::containers::ModBlockEntity,
    max: i64,
    simulate: bool,
    accepted: &mut Option<i64>,
) {
    crate::game::mi_world::fe_receive(entity, max, simulate, accepted);
}
