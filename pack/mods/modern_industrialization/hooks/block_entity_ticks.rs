// MI's machines tick.
fn mi_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::mi_world::ticks(block, ticks);
}
