// The world closes (or fails to open): the parsed recipes and the machines' unsaved state are dropped.
fn theurgy_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::theurgy_world::lifecycle(phase);
}
