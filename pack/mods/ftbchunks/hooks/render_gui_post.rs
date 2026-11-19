// The corner minimap, each frame the HUD is drawn (its image is made in `render_frame_pre`).
fn ftbchunks_render_gui_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    crate::game::ftbchunks_minimap::hud(game, batch, corners);
}
