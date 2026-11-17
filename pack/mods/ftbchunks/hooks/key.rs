// M opens and closes the claim map; FTB Chunks' minimap toggle key.
fn ftbchunks_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::ftbchunks_largemap::key(game, key, pressed);
    crate::game::ftbchunks_map::key(game, key, pressed);
    crate::game::ftbchunks_minimap::key(game, key, pressed);
}
