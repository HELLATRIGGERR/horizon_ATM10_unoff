// A player right-clicks with an item: RIGHT_CLICK_ITEM.
fn ftbchunks_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::ftbchunks_world::use_item(game, stack, claimed);
}
