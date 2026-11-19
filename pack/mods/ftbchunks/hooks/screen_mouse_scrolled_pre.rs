// The wheel: the large map zooms; on the claim screen it moves a force-loaded chunk's expiry.
fn ftbchunks_screen_mouse_scrolled_pre(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::ftbchunks_map::scrolled(game, screen, scroll, canceled);
    crate::game::ftbchunks_largemap::scrolled(game, screen, scroll, canceled);
}
