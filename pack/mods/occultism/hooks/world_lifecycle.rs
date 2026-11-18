// A world opens or closes: Occultism's parsed recipes and pentacles are dropped.
fn occultism_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::occultism_world::lifecycle(phase);
}
