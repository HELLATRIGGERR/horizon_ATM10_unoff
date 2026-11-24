// The world closes (or fails to open): the recipe index is dropped.
fn woodworking_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::woodworking_world::lifecycle(phase);
}
