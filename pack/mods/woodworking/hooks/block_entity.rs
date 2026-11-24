// The woodworking station gets its block entity.
fn woodworking_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::woodworking_world::new_entity(block, entity);
}
