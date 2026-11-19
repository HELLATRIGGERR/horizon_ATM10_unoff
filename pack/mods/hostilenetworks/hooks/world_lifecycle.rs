// A world opens or closes: the settings and the registry of data models are read afresh or dropped.
fn hostilenetworks_world_lifecycle(
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::hostilenetworks_world::lifecycle(phase);
}
