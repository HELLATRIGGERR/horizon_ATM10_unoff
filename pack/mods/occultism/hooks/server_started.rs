// The server opened its world: Occultism's parsed recipes and pentacles are dropped, read again on first use.
fn occultism_server_started() {
    crate::game::occultism_world::lifecycle(None);
}
