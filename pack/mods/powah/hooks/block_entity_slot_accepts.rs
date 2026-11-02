// What a Furnator's slots take from hoppers and clicks.
fn powah_block_entity_slot_accepts(block: &str, slot: usize, fuel: bool, accepts: &mut bool) {
    crate::game::powah_world::slot_accepts(block, slot, fuel, accepts);
}
