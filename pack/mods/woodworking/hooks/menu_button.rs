// A button press on the woodworking station, on the server: execute the selected recipe.
fn woodworking_menu_button(
    game: &mut crate::game::Game<'_>,
    context: &mod_api::item::menu::MenuButtonContext,
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::woodworking_world::button(game, entity, context.id);
}
