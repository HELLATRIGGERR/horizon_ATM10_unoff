// What a machine's slots take from hoppers and clicks.
fn hostilenetworks_block_entity_slot_accepts(
    block: &str,
    slot: usize,
    accepts: &mut bool,
) {
    crate::game::hostilenetworks_world::slot_accepts(block, slot, accepts);
}
