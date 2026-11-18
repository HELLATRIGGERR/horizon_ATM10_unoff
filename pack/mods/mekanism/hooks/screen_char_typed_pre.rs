// A character typed on a QIO screen: its search bar or frequency name.
fn mek_screen_char_typed_pre(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    crate::game::mek_qio::char_typed(game, c, canceled);
}
