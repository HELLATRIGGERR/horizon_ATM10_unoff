// A golden bowl's ritual, one server tick.
fn occultism_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::occultism_world::tick(game, pos, entity);
}
