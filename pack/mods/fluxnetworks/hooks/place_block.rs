// A player placed a block: a plug or a point takes what its item kept, and its owner.
fn flux_place_block(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
) {
    crate::game::flux_world::placed(game, stack, pos);
}
