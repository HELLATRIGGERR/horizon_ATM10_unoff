// After a level's tick: rituals that ended, items in spirit fire.
fn occultism_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::occultism_world::level_ticked(game, dimension);
}
