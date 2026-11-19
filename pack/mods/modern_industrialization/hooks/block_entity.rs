// An MI machine gets its block entity (`MachineBlock.newBlockEntity`).
fn mi_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::mi_world::new_entity(block, entity);
}
