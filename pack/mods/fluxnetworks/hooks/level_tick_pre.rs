// The level whose block entities tick next.
fn flux_level_tick_pre(level: mod_api::LevelRef) {
    crate::game::flux_world::level_pre(level.dimension_id());
}
