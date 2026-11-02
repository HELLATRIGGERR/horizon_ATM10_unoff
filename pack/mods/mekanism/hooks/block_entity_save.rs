// A Mekanism block entity is written for saving: a cable's share of its network.
fn mek_block_entity_save(dimension: &str, pos: [i32; 3], id: &str, nbt: &mut horizon_world::nbt::Compound) {
    crate::game::mek_world::entity_saved(dimension, pos, id, nbt);
}
