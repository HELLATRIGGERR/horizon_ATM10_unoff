// A press while a QIO frequency select screen covers its block's screen: the select screen's, nothing under it.
fn mek_screen_mouse_button_pressed_pre(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::mek_qio::select_pressed(game, screen, button, canceled);
}
