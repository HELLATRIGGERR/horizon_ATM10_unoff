// The server opened its world: the bookmarks learn which folder is the open world's.
fn jei_server_started(game: &mut crate::game::Game<'_>) {
    crate::game::jei_bookmarks::opened(game);
}
