// A world opens or closes: MI's settings and recipe books are read again.
fn mi_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::mi_world::lifecycle(phase);
}
