// The claim screen, the large map, or the sidebar buttons on the inventory are drawn.
fn ftbchunks_screen_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::ftbchunks_map::render(game, batch, screen);
    crate::game::ftbchunks_largemap::render(game, batch, screen);
}
