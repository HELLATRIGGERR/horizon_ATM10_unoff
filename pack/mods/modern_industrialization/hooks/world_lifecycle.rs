// The world closes (or fails to open): MI's settings and recipe books are dropped.
fn mi_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::mi_world::lifecycle(phase);
}
