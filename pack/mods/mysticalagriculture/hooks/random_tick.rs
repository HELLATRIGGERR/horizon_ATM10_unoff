// A random tick of one of MA's crops: one that can't grow where it stands (its crux, its farmland) takes none.
fn ma_random_tick(
    level: &mut crate::game::GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    handled: &mut bool,
) {
    let Some(block) = level.native_state_block(state).map(str::to_owned) else { return };
    if !block.starts_with("mysticalagriculture:") {
        return;
    }
    let below = crate::game::ma_world::level_block(level, [pos.x, pos.y - 1, pos.z]);
    let below2 = crate::game::ma_world::level_block(level, [pos.x, pos.y - 2, pos.z]);
    crate::game::ma_world::random_tick(&block, &below, &below2, handled);
}
