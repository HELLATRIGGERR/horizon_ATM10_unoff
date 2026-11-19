// A Simulation Chamber or a Loot Fabricator gets its block entity (`EntityBlock.newBlockEntity`).
fn hostilenetworks_block_entity(
    block: &str,
    entity: &mut Option<(String, horizon_world::nbt::Compound)>,
) {
    crate::game::hostilenetworks_world::new_entity(block, entity);
}
