// Draws a Mekanism block entity on a client: a transporter's travelling stacks, a QIO Drive Array's drives.
fn mek_block_entity_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::mek_transmit::draw(game, pos, block, entity, draw);
    crate::game::mek_qio::draw_block(game, pos, block, entity, draw);
}
