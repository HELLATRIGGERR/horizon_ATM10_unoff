// The server opened its worlds, before any player joins: the teams are read.
fn ftbteams_server_started(game: &mut crate::game::Game<'_>) {
    crate::game::ftbteams_world::opened(game);
}
