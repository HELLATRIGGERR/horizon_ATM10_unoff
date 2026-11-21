// A button of a Mekanism block's screen, on the server; the QIO item viewer's clicks take the player's cursor stack.
fn mek_menu_button(
    game: &mut crate::game::Game<'_>,
    context: &mod_api::item::menu::MenuButtonContext,
    entity: &mut mod_api::ModBlockEntity,
    carried: &mut Option<mod_api::ItemStack>,
) {
    crate::game::mek_world::api_button(game, context.pos, entity, context.id, carried);
}
