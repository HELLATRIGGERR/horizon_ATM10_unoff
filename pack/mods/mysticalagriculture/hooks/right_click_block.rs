// A right click on the infusion altar or a pedestal: an item goes on or comes off; the wand activates the altar.
fn ma_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::ma_world::right_click(game, pos, canceled);
}
