// What clients get of a Mekanism block entity: a transporter's travelling stacks, a QIO Drive Array's drive lights.
fn mek_block_entity_update_tag(pos: [i32; 3], entity: &mod_api::ModBlockEntity, tag: &mut Option<horizon_core::nbt::Compound>) {
    let _ = pos;
    let (engine, _) = crate::game::mek_world::engine_entity(entity, None);
    crate::game::mek_transmit::update_tag(&engine, tag);
    crate::game::mek_qio::update_tag(&engine, tag);
}
