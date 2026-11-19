// The server opened its world: the networks and settings are read.
fn flux_server_started(game: &mut crate::game::Game<'_>) {
    crate::game::flux_world::opened(game);
}
