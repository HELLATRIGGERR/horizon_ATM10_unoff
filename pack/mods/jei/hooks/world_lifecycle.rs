// The world closes (or fails to open) on the server thread: the bookmarks are no longer the world's.
fn jei_world_lifecycle(phase: mod_api::WorldPhase) {
    let _ = phase;
    crate::game::jei_bookmarks::closed();
}
