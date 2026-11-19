// The infusion altar, the pedestals and the Growth Accelerators get their block entities.
fn ma_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::ma_world::new_entity(block, entity);
}
