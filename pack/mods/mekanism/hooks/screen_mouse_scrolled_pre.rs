// The wheel on a QIO dashboard: its item viewer scrolls.
fn mek_screen_mouse_scrolled_pre(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    crate::game::mek_qio::scrolled(game, scroll, canceled);
}
