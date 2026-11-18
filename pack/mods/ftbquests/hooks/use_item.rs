// A right click with the book item opens the book.
fn ftbquests_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::ftbquests_world::use_item(game, stack, claimed);
}
