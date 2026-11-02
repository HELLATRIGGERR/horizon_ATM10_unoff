// A player placed a block from an item: a cube faces them and takes the item's energy.
fn mek_place_block(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3]) {
    crate::game::mek_world::placed(game, stack, pos);
}
