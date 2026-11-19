// A server tick of a Furnator.
fn powah_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::powah_world::tick(game, pos, entity);
}
