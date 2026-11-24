// The woodworking station entity ticks.
fn woodworking_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::woodworking_world::ticks(block, ticks);
}
