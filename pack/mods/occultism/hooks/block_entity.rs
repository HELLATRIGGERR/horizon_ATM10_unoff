// An Occultism bowl gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn occultism_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::occultism_world::new_entity(block, entity);
}
