// After a level's tick: cables; forget breaks that dropped nothing, and look at the config files; keep the level's
// radiation sources.
fn mek_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::mek_world::level_ticked(game, dimension);
    crate::game::mek_radiation::level_ticked(game, dimension);
}
