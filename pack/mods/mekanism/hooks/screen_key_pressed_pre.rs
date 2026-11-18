// A key pressed on a QIO screen while one of its fields has the focus.
fn mek_screen_key_pressed_pre(game: &crate::game::Game<'_>, key: &str, canceled: &mut bool) {
    crate::game::mek_qio::key_pressed(game, key, canceled);
}
