// The bookmark list left of the open screen, then JEI's recipe window over it.
fn jei_screen_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    if game.native_on(crate::game::jei_world::ID) {
        crate::game::jei_bookmarks::render(game, batch, screen);
        crate::game::jei_window::render(game, batch, screen);
    }
}
