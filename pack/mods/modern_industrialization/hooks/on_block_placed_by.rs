// A multiblock controller placed by a player faces them (`OrientationComponent.onPlaced`; the hook passes one call).
fn mi_on_block_placed_by(game: &mut crate::game::Game<'_>, call: &super::fire::OnBlockPlacedByCall<'_>) {
    crate::game::mi_multiblock::placed(game, call.pos, call.placer.as_ref());
}
