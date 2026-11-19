// The world closes (or fails to open): the settings and the registry of data models are dropped.
fn hostilenetworks_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::hostilenetworks_world::lifecycle(phase);
}
