// What clients get of a Mekanism block entity: a transporter's travelling stacks, a QIO Drive Array's drive lights.
fn mek_block_entity_update_tag(pos: [i32; 3], entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
    let _ = pos;
    crate::game::mek_transmit::update_tag(entity, tag);
    crate::game::mek_qio::update_tag(entity, tag);
}
