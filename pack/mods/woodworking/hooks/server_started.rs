// The server opened its world: drop the cached index so it is rebuilt from the new world's tags.
fn woodworking_server_started() {
    crate::game::woodworking_world::opened();
}
