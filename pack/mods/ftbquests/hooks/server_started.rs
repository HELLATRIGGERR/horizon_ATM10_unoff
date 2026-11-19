// The server opened its worlds, before any player joins: the teams' progress is read.
fn ftbquests_server_started(game: &mut crate::game::Game<'_>) {
    crate::game::ftbquests_world::opened(game);
}
