// Every processing machine ticks.
fn oritech_block_entity_ticks(
    block: &str,
    ticks: &mut bool,
) {
    crate::game::oritech_world::ticks(block, ticks);
}
