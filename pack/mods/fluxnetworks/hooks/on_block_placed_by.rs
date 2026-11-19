// A player placed a block: a plug or a point takes what its item kept, and its owner (the hook passes one call).
fn flux_on_block_placed_by(game: &mut crate::game::Game<'_>, call: &super::fire::OnBlockPlacedByCall<'_>) {
    crate::game::flux_world::placed(game, call.stack, call.pos);
}
