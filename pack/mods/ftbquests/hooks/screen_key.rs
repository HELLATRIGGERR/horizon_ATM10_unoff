// A key over the book.
fn ftbq_screen_key(game: &crate::game::Game<'_>, key: &str, canceled: &mut bool) {
    crate::game::ftbq_ui::key_over(game, key, canceled);
}
