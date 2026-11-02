// After a level's tick: cables; forget breaks that dropped nothing, and look at the config files.
fn mek_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::mek_world::level_ticked(game, dimension);
}
