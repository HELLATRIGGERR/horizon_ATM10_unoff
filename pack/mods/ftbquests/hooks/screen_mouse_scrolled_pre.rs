// The wheel scrolls the chapters or zooms the grid.
fn ftbquests_screen_mouse_scrolled_pre(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::ftbquests_book::scrolled(game, screen, scroll, canceled);
}
