// The generator screen's redstone button, on the server.
fn justdirethings_menu_button(
    game: &mut crate::game::Game<'_>,
    context: &mod_api::item::menu::MenuButtonContext,
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::justdirethings_world::button(game, entity, context.id);
}
