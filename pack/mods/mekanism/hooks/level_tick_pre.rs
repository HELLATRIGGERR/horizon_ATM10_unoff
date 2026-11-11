// Before a level's tick: Mekanism's multiblocks tick, as their master casing ticks before the blocks placed after it;
// the level's radiation sources decay now and then.
fn mek_level_tick_pre(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::mek_multiblock::level_tick_pre(game, dimension);
    crate::game::mek_radiation::level_tick_pre(game, dimension);
}
