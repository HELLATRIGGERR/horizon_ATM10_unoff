// A new apparatus' block entity: its saved keys, empty.
fn theurgy_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::theurgy_world::new_entity(block, entity);
}
