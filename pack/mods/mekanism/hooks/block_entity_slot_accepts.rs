// What a Mekanism block's slot takes: the heat generator's fuel slot, what burns; a machine's slots, what they work with.
fn mek_block_entity_slot_accepts(block: &str, slot: usize, stack: &horizon_client::items::ItemStack, fuel: bool, accepts: &mut bool) {
    crate::game::mek_world::slot_accepts(block, slot, stack, fuel, accepts);
}
