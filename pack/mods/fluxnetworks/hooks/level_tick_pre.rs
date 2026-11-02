// The level whose block entities tick next.
fn flux_level_tick_pre(dimension: &str) {
    crate::game::flux_world::level_pre(dimension);
}
