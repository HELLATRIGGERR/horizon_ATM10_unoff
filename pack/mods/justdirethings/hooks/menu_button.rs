// A machine screen's buttons, on the server: the generator's and the breaker's redstone, the Paradox Machine's
// settings and snapshot.
fn justdirethings_menu_button(
    game: &mut crate::game::Game<'_>,
    context: &mod_api::item::menu::MenuButtonContext,
    entity: &mut mod_api::ModBlockEntity,
) {
    crate::game::justdirethings_world::button(game, context.pos, entity, context.id);
}
