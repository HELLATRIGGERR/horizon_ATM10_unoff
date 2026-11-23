// A two-block apparatus placed: its upper half (`setPlacedBy`).
fn theurgy_on_block_placed_by(game: &mut crate::game::Game<'_>, call: &super::fire::OnBlockPlacedByCall<'_>) {
    crate::game::theurgy_world::placed(game, call.pos);
}
