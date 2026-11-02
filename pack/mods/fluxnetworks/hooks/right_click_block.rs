// A player right clicks a block: a Flux device opens its menu.
fn flux_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::flux_world::right_click(game, pos, canceled);
}
