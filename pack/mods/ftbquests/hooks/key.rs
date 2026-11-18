// The quests key opens and closes the book.
fn ftbquests_key(
    game: &mut crate::game::Game<'_>,
    key: &str,
    pressed: bool,
) {
    crate::game::ftbquests_book::key(game, key, pressed);
}
