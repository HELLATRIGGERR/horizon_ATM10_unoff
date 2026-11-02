// The mod's key opens the book.
fn ftbq_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::ftbq_ui::key(game, key, pressed);
}
