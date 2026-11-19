// The bookmark list left of the open screen, then JEI's recipe window over it, then JEI's config screen.
fn jei_screen_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &mod_api::ScreenView<'_>,
) {
    if game.native_on(crate::game::jei_world::ID) {
        crate::game::jei_bookmarks::render(game, batch, screen);
        crate::game::jei_window::render(game, batch, screen);
        crate::game::jei_config::render(game, batch, screen);
    }
}
