// A Mekanism block entity entered the world: cables join their network.
fn mek_block_entity_load(dimension: &str, pos: [i32; 3], entity: &mut mod_api::ModBlockEntity) {
    crate::game::mek_world::api_entity_loaded(dimension, pos, entity);
}
