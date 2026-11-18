// Over the whole screen, its slots' items included: a QIO frequency select screen.
fn mek_screen_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::mek_qio::draw_select(game, batch, screen);
}
