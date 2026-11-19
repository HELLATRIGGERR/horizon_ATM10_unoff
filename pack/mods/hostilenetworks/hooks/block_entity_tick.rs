// A server tick of a Simulation Chamber or a Loot Fabricator.
fn hostilenetworks_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::hostilenetworks_world::tick(game, pos, entity);
}
