// A player fills a bucket: EDIT_FLUID.
fn ftbchunks_fill_bucket(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::fill_bucket(game, pos, canceled);
}
