// The brazier and the crafting apparatus tick.
fn theurgy_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::theurgy_world::ticks(block, ticks);
}
