// A Mekanism block entity leaves the world: cables leave their network.
fn mek_block_entity_unload(dimension: &str, pos: [i32; 3], entity: &mod_api::ModBlockEntity, removed: bool) {
    crate::game::mek_world::api_entity_unloaded(dimension, pos, entity, removed);
}
