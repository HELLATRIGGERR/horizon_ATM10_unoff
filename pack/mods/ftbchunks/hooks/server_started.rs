// The server opened its worlds, before any player joins: the claims are read.
fn ftbchunks_server_started(game: &mut crate::game::Game<'_>) {
    crate::game::ftbchunks_world::opened(game);
}
