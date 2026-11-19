// A button of a machine's screen, on the server.
fn hostilenetworks_menu_button(
    game: &mut crate::game::Game<'_>,
    context: &mod_api::item::menu::MenuButtonContext,
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::hostilenetworks_world::button(game, entity, context.id);
}
