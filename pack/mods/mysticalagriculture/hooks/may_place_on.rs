// The ground one of MA's crops stands on: any farmland, MA's essence farmland among it (`CropBlock.mayPlaceOn`,
// `instanceof FarmBlock`).
fn ma_may_place_on(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    ground: horizon_world::registry::BlockStateId,
    may: &mut Option<bool>,
) {
    let Some(block) = level.native_state_block(state) else { return };
    if !block.starts_with("mysticalagriculture:") {
        return;
    }
    let ground = level.native_state_block(ground).unwrap_or("minecraft:air");
    crate::game::ma_world::may_place_on(block, ground, may);
}
