// A Mekanism block entity entered the world: cables join their network.
fn mek_block_entity_load(dimension: &str, pos: [i32; 3], entity: &mut horizon_client::containers::ModBlockEntity) {
    crate::game::mek_world::entity_loaded(dimension, pos, entity);
}
