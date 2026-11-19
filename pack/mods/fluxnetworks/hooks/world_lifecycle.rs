// The world closes (or fails to open): the networks and settings are dropped.
fn flux_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::flux_world::lifecycle(phase);
}
