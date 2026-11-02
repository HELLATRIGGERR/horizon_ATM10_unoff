// Draws the book.
fn ftbq_screen_render_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    crate::game::ftbq_ui::draw(game, batch, screen);
}
