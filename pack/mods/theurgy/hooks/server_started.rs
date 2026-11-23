// The server opened its world: the parsed recipes and the machines' unsaved state are dropped.
fn theurgy_server_started() {
    crate::game::theurgy_world::opened();
}
