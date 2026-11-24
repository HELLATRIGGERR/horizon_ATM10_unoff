// A block was set: a multiblock whose shape holds it matches again (`ShapeMatcher.onBlockUpdate`).
fn mi_block_changed(pos: horizon_core::position::BlockPos) {
    crate::game::mi_multiblock::block_changed([pos.x, pos.y, pos.z]);
}
