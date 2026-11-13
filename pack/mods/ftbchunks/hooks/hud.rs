// The corner minimap, each frame the HUD is drawn.
fn ftbchunks_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    crate::game::ftbchunks_minimap::hud(game, batch, corners);
}
