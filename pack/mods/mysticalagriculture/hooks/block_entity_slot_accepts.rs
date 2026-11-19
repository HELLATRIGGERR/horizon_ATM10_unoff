// Nothing goes into the altar's output slot.
fn ma_block_entity_slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    crate::game::ma_world::slot_accepts(block, slot, accepts);
}
