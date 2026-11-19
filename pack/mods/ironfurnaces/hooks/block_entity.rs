// A furnace gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn ironfurnaces_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::ironfurnaces_world::new_entity(block, entity);
}
