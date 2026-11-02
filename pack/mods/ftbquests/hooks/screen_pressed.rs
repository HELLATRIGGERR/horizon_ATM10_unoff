// A press on the book.
fn ftbq_screen_pressed(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    crate::game::ftbq_ui::pressed(game, screen, button, canceled);
}
