// A world opens or closes: MA's settings and the pack's custom crops are read or dropped.
fn ma_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::ma_world::lifecycle(phase);
}
