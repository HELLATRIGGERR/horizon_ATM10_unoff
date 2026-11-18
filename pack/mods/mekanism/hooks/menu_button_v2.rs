// A button of a Mekanism block's screen with the player's cursor stack: the QIO item viewer's clicks.
fn mek_menu_button_v2(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
    id: i32,
    carried: &mut Option<horizon_client::items::ItemStack>,
) {
    crate::game::mek_qio::button(game, pos, entity, id, carried);
}
