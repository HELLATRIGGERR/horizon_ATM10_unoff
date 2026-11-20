// A server tick of a Mekanism block entity: a cube or a generator.
fn mek_block_entity_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut mod_api::ModBlockEntity) {
    crate::game::mek_world::api_tick(game, pos, entity);
}
