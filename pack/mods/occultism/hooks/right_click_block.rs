// A right click on a block: the bowls' slots, a golden bowl's ritual, spirit fire lit over datura.
fn occultism_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    crate::game::occultism_world::right_click(game, pos, face, canceled);
}
