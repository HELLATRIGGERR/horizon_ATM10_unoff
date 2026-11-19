// A Furnator placed from its item takes the energy back (`BlockItem.onBlockPlacedBy`; the hook passes one call).
fn powah_on_block_placed_by(game: &mut crate::game::Game<'_>, call: &super::fire::OnBlockPlacedByCall<'_>) {
    crate::game::powah_world::placed(game, call.stack, call.pos);
}
