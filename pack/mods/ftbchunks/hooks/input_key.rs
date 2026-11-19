// M opens and closes the claim map; FTB Chunks' minimap toggle key (presses only, as the fork's `key`).
fn ftbchunks_input_key(game: &mut crate::game::Game<'_>, key: &str, action: mod_api::KeyAction) {
    let pressed = matches!(action, mod_api::KeyAction::Press);
    crate::game::ftbchunks_largemap::key(game, key, pressed);
    crate::game::ftbchunks_map::key(game, key, pressed);
    crate::game::ftbchunks_minimap::key(game, key, pressed);
}
