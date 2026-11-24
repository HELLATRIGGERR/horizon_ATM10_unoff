// What a machine's slots take from hoppers and clicks: its inputs.
fn oritech_block_entity_slot_accepts(
    block: &str,
    slot: usize,
    accepts: &mut bool,
) {
    crate::game::oritech_world::slot_accepts(block, slot, accepts);
}
