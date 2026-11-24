// A server tick of a goo block or a Generator T1.
fn justdirethings_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::justdirethings_world::tick(game, pos, entity);
}
