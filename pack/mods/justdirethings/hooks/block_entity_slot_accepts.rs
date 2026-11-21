// The generator's slot takes fuel only (`FuelSlot`).
fn justdirethings_block_entity_slot_accepts(
    block: &str,
    slot: usize,
    fuel: bool,
    accepts: &mut bool,
) {
    crate::game::justdirethings_world::slot_accepts(block, slot, fuel, accepts);
}
