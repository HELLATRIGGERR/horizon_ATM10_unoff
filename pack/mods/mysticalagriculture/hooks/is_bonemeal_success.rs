// Bone meal never grows one of MA's crops (`MysticalCropBlock.isBonemealSuccess`).
fn ma_is_bonemeal_success(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    success: &mut bool,
) {
    if let Some(block) = level.native_state_block(state).map(str::to_owned) {
        crate::game::ma_world::bonemeal_success(&block, success);
    }
}
