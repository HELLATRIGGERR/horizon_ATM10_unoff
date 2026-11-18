// A click on the book.
fn ftbquests_screen_mouse_button_pressed_pre(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    let _ = modifiers;
    crate::game::ftbquests_book::pressed(game, screen, button, canceled);
}
