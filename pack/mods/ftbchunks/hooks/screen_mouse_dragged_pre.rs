// The pointer moves with a button held over the claim map.
fn ftbchunks_screen_mouse_dragged_pre(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    crate::game::ftbchunks_map::dragged(game, screen, canceled);
    crate::game::ftbchunks_largemap::dragged(game, screen, canceled);
}
