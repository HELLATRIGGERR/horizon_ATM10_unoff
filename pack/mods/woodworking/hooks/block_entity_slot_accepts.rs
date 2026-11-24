// What the woodworking station's slots accept (input accepts any item; output is read-only).
fn woodworking_block_entity_slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    crate::game::woodworking_world::slot_accepts(block, slot, accepts);
}
