// Draws a Mekanism block entity on a client: a transporter's travelling stacks, a QIO Drive Array's drives.
fn mek_block_entity_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &mod_api::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    let (engine, _) = crate::game::mek_world::engine_entity(entity, None);
    crate::game::mek_transmit::draw(game, pos, block, &engine, draw);
    crate::game::mek_qio::draw_block(game, pos, block, &engine, draw);
}
