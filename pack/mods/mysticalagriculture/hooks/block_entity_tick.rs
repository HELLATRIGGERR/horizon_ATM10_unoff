// A server tick of the infusion altar or a Growth Accelerator.
fn ma_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::ma_world::tick(game, pos, entity);
}
