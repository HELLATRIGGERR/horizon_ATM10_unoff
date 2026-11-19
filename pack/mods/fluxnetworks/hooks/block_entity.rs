// A Flux device gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn flux_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::flux_world::new_entity(block, entity);
}
