// A Mekanism block gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn mek_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::mek_world::new_entity(block, entity);
}
