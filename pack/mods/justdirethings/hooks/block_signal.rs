// A block's redstone signal is read: a Sensor T1 gives 15 on every side but its front while it senses something, as its
// strong signal too when set so (`SensorT1.getSignal`, `getDirectSignal`).
fn justdirethings_block_signal(
    level: &crate::game::GameLevel<'_>,
    dimension: crate::game::LevelDimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    face: horizon_core::geometry::Direction,
    direct: bool,
    signal: &mut i32,
) {
    crate::game::justdirethings_more::block_signal(dimension.key().id(), level.native_state_block(state), [pos.x, pos.y, pos.z], face, direct, signal);
}
