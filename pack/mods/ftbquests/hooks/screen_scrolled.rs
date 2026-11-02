// The wheel over the book.
fn ftbq_screen_scrolled(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, scroll: [f64; 2]) {
    crate::game::ftbq_ui::scrolled(game, screen, scroll);
}
