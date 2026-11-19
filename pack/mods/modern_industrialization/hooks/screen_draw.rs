// Draws an MI machine's screen (`MachineScreen.renderBg`).
fn mi_screen_draw(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::mi_ui::draw(game, block, data, batch, origin);
}
