// The server opened its world: the settings are read afresh, the recipe books and the unsaved state dropped.
fn oritech_server_started() {
    crate::game::oritech_world::opened();
}
