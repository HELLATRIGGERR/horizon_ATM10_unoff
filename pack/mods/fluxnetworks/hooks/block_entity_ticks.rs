// Whether a Flux block's entity ticks.
fn flux_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::flux_world::ticks(block, ticks);
}
