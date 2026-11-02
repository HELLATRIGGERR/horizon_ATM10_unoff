// Furnators tick.
fn powah_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::powah_world::ticks(block, ticks);
}
