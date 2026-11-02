// What a Mekanism block's slot takes: the heat generator's fuel slot, what burns.
fn mek_block_entity_slot_accepts(block: &str, slot: usize, fuel: bool, accepts: &mut bool) {
    crate::game::mek_world::slot_accepts(block, slot, fuel, accepts);
}
