// A block was set: the cables beside it look at their connections again.
fn mek_block_changed(pos: horizon_core::position::BlockPos) {
    crate::game::mek_world::block_set([pos.x, pos.y, pos.z]);
}
