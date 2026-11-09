// A player right clicks a block, before the block acts: a configurator in its rotate mode turns a Mekanism block;
// a multiblock's block opens its screen only when formed.
fn mek_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    let _ = face;
    crate::game::mek_configurator::right_click(game, pos, canceled);
    crate::game::mek_multiblock::right_click(game, pos, canceled);
}
