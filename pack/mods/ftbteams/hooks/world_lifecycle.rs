// A world closes (or fails to open): the teams are written and dropped.
fn ftbteams_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::ftbteams_world::lifecycle(phase);
}
