// The world closes (or fails to open): Occultism's parsed recipes and pentacles are dropped.
fn occultism_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::occultism_world::lifecycle(Some(phase));
}
