// What an MI machine's slots take from hoppers and clicks: inputs anything, outputs nothing.
fn mi_block_entity_slot_accepts(block: &str, slot: usize, fuel: bool, accepts: &mut bool) {
    crate::game::mi_world::slot_accepts(block, slot, fuel, accepts);
}
