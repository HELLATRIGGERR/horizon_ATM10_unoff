// The quests key opens and closes the book (presses only, as the fork's `key`).
fn ftbquests_input_key(game: &mut crate::game::Game<'_>, key: &str, action: mod_api::KeyAction) {
    crate::game::ftbquests_book::key(game, key, matches!(action, mod_api::KeyAction::Press));
}
