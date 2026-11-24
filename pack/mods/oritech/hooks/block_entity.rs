// A processing machine gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn oritech_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::oritech_world::new_entity(block, entity);
}
