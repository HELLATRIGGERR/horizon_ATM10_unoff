// Fire or a fluid spreads into the next block: not across a claim's border when the config says so.
fn ftbchunks_block_spread(
    dimension: crate::game::LevelDimension,
    kind: &str,
    from: horizon_core::position::BlockPos,
    to: horizon_core::position::BlockPos,
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::block_spread(dimension.key().id(), kind, from, to, canceled);
}
