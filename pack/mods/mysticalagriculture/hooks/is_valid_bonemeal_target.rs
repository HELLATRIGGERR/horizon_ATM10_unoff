// Bone meal on one of MA's crops: a target where it can grow and isn't ripe.
fn ma_is_valid_bonemeal_target(
    level: &crate::game::GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    valid: &mut Option<bool>,
) {
    let Some(block) = level.native_state_block(state).map(str::to_owned) else { return };
    if !block.starts_with("mysticalagriculture:") {
        return;
    }
    let age = level
        .native_state_with_property(state, "age", "7")
        .map(|ripe| if ripe == state { "7" } else { "0" });
    let below = crate::game::ma_world::level_block(level, [pos.x, pos.y - 1, pos.z]);
    let below2 = crate::game::ma_world::level_block(level, [pos.x, pos.y - 2, pos.z]);
    crate::game::ma_world::valid_bonemeal_target(&block, age, &below, &below2, valid);
}
