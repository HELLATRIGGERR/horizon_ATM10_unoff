// The world closes (or fails to open): MA's settings and the pack's custom crops are dropped.
fn ma_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::ma_world::lifecycle(phase);
}
