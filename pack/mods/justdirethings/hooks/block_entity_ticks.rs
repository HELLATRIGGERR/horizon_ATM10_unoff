// The goo blocks and the generator tick.
fn justdirethings_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::justdirethings_world::ticks(block, ticks);
}
