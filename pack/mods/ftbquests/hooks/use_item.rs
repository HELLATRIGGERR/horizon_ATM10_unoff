// Right click with an item: the quest book opens.
fn ftbq_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::ftbq_world::use_item(game, stack, claimed);
}
