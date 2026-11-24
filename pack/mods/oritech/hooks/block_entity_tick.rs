// A server tick of a processing machine.
fn oritech_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::oritech_world::tick(game, pos, entity);
}
