// The infusion altar, the pedestals and the Growth Accelerators get their block entities; the declaration owns
// their type ids.
fn ma_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::ma_world::new_entity(block, entity);
}
