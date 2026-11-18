// Furnaces tick.
fn ironfurnaces_block_entity_ticks(
    block: &str,
    ticks: &mut bool,
) {
    crate::game::ironfurnaces_world::ticks(block, ticks);
}
