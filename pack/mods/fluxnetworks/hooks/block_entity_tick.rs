// A server tick of a plug or a point.
fn flux_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::flux_world::tick(game, pos, entity);
}
