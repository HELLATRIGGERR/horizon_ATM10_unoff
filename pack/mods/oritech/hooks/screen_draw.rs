// Draws a machine's screen.
fn oritech_screen_draw(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::oritech_ui::draw(game, block, data, batch, origin);
}
