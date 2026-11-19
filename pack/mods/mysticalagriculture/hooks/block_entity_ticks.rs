// The infusion altar and the Growth Accelerators tick.
fn ma_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::ma_world::ticks(block, ticks);
}
