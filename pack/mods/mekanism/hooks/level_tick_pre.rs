// Before a level's tick: Mekanism's multiblocks tick, as their master casing ticks before the blocks placed after it.
fn mek_level_tick_pre(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::mek_multiblock::level_tick_pre(game, dimension);
}
