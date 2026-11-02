// A Furnator gets its block entity (`EntityBlock.newBlockEntity`).
fn powah_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::powah_world::new_entity(block, entity);
}
