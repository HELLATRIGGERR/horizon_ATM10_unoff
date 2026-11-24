// A server tick of the woodworking station.
fn woodworking_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::woodworking_world::tick(game, pos, entity);
}
