// Each frame: the map's tiles, the requests to send, and the minimap's image.
fn ftbchunks_render_frame_pre(game: &mut crate::game::Game<'_>) {
    crate::game::ftbchunks_map::frame(game);
    crate::game::ftbchunks_largemap::frame(game);
    crate::game::ftbchunks_minimap::frame(game);
}
