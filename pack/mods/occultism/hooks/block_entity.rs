// An Occultism bowl gets its block entity (`EntityBlock.newBlockEntity`).
fn occultism_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::occultism_world::new_entity(block, entity);
}
