// A Furnator gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn powah_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::powah_world::new_entity(block, entity);
}
