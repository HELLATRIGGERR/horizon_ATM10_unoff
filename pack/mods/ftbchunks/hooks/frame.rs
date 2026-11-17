// Each frame: the map's tiles, and the requests to send.
fn ftbchunks_frame(game: &mut crate::game::Game<'_>) {
    crate::game::ftbchunks_map::frame(game);
    crate::game::ftbchunks_largemap::frame(game);
}
