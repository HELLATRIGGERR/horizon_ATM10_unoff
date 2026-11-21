// A player placed a block from an item: a cube faces them and takes the item's energy (`BlockItem.onBlockPlacedBy`;
// the hook passes one call).
fn mek_on_block_placed_by(game: &mut crate::game::Game<'_>, call: &super::fire::OnBlockPlacedByCall<'_>) {
    crate::game::mek_world::placed(game, call.stack, call.pos);
}
