// A furnace gets its block entity (`EntityBlock.newBlockEntity`).
fn ironfurnaces_block_entity(
    block: &str,
    entity: &mut Option<(String, horizon_world::nbt::Compound)>,
) {
    crate::game::ironfurnaces_world::new_entity(block, entity);
}
