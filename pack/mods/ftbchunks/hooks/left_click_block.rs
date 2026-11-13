// A player starts breaking a block: EDIT_BLOCK.
fn ftbchunks_left_click_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::destroy_block(game, pos, canceled);
}
