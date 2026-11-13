// A player right-clicks a block: INTERACT_BLOCK.
fn ftbchunks_right_click_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::right_click_block(game, pos, canceled);
}
