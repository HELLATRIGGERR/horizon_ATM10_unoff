// Golden bowls tick their rituals.
fn occultism_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::occultism_world::ticks(block, ticks);
}
