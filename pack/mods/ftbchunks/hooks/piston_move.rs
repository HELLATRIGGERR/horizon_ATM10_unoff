// A piston moves blocks: none crosses into another team's claim.
fn ftbchunks_piston_move(
    dimension: crate::game::LevelDimension,
    piston: horizon_core::position::BlockPos,
    direction: [i32; 3],
    push: &[horizon_core::position::BlockPos],
    destroy: &[horizon_core::position::BlockPos],
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::piston_move(dimension.key().id(), piston, direction, push, destroy, canceled);
}
