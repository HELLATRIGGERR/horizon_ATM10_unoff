// A two-block apparatus placed: its upper half (`setPlacedBy`); a sulfuric flux emitter: the pedestals its placer picked.
fn theurgy_on_block_placed_by(game: &mut crate::game::Game<'_>, call: &super::fire::OnBlockPlacedByCall<'_>) {
    crate::game::theurgy_world::placed(game, call.pos, call.placer.as_ref().map(|p| p.uuid()));
}
