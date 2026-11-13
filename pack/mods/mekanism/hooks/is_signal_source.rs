// Whether a block gives redstone: a fission reactor's logic adapter does, so wire beside it connects.
fn mek_is_signal_source(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, source: &mut bool) {
    let block = level.native_state_block(state);
    crate::game::mek_fission::is_signal_source(block, source);
    crate::game::mek_lasers::is_signal_source(block, source);
}
