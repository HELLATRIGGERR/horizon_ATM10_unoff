// Both machines tick.
fn hostilenetworks_block_entity_ticks(
    block: &str,
    ticks: &mut bool,
) {
    crate::game::hostilenetworks_world::ticks(block, ticks);
}
