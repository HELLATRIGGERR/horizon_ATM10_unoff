// A Mekanism block entity leaves the world: cables leave their network.
fn mek_block_entity_unload(dimension: &str, pos: [i32; 3], entity: &horizon_client::containers::ModBlockEntity, removed: bool) {
    crate::game::mek_world::entity_unloaded(dimension, pos, entity, removed);
}
