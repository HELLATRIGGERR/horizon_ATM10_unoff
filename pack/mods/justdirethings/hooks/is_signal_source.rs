// Whether a block gives redstone: a Sensor T1 does, so wire beside it connects.
fn justdirethings_is_signal_source(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, source: &mut bool) {
    crate::game::justdirethings_more::is_signal_source(level.native_state_block(state), source);
}
