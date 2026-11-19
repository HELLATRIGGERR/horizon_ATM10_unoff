// The world closes (or fails to open): the teams' progress is written and dropped.
fn ftbquests_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::ftbquests_world::lifecycle(phase);
}
