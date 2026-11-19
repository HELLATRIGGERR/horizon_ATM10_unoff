// A server tick of a furnace.
fn ironfurnaces_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::ironfurnaces_world::tick(game, pos, entity);
}
