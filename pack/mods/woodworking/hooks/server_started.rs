// The server opened its world: the index is built again from the new world's recipes and tags.
fn woodworking_server_started(game: &mut crate::game::Game<'_>) {
    crate::game::woodworking_world::opened(game);
}
