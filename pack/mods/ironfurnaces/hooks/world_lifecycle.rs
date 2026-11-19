// The world closes (or fails to open): the furnaces' settings are dropped.
fn ironfurnaces_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::ironfurnaces_world::lifecycle(phase);
}
