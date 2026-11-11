// A block's redstone signal is read: a fission reactor's logic adapter gives 15 out of its reactor while its status
// is outputting (`AttributeRedstoneEmitter`).
fn mek_block_signal(
    level: &crate::game::GameLevel<'_>,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    face: horizon_core::geometry::Direction,
    direct: bool,
    signal: &mut i32,
) {
    let name = match dimension {
        horizon_world::level::Dimension::Overworld => "minecraft:overworld",
        horizon_world::level::Dimension::Nether => "minecraft:the_nether",
        horizon_world::level::Dimension::End => "minecraft:the_end",
    };
    crate::game::mek_fission::block_signal(name, level.native_state_block(state), [pos.x, pos.y, pos.z], face.normal(), direct, signal);
}
