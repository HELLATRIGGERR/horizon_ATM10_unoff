// Draws the generator's screen.
fn justdirethings_screen_draw(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::justdirethings_ui::draw(game, block, data, batch, origin);
}
