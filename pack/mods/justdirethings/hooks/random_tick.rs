// A random tick of the time crystal's budding block: its stage and its clusters (not budding amethyst's growth).
fn justdirethings_random_tick(
    level: &mut crate::game::GameLevel<'_>,
    ticks: &mut horizon_world::level::Ticks,
    dimension: horizon_core::dimension::Dimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    handled: &mut bool,
) {
    crate::game::justdirethings_time::random_tick(level, ticks, dimension, pos, state, handled);
}
