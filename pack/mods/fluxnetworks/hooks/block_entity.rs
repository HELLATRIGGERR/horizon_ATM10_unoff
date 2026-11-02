// A Flux device gets its block entity (`EntityBlock.newBlockEntity`).
fn flux_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::flux_world::new_entity(block, entity);
}
