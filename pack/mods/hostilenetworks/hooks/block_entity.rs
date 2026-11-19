// A Simulation Chamber or a Loot Fabricator gets its block entity (`EntityBlock.newBlockEntity`); the declaration
// owns its type id.
fn hostilenetworks_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::hostilenetworks_world::new_entity(block, entity);
}
