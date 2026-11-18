// What a furnace's slots take from hoppers and clicks.
fn ironfurnaces_block_entity_slot_accepts(
    block: &str,
    slot: usize,
    fuel: bool,
    accepts: &mut bool,
) {
    crate::game::ironfurnaces_world::slot_accepts(block, slot, fuel, accepts);
}
