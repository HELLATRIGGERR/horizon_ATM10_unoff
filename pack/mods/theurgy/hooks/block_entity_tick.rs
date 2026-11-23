// One server tick of an apparatus.
fn theurgy_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::theurgy_world::tick(game, pos, entity);
}
