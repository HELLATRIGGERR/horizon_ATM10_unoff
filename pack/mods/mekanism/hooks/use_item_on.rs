// A player uses an item on a block: a tier installer turns a machine into a factory, a factory into the next tier.
fn mek_use_item_on(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3], claimed: &mut bool) {
    crate::game::mek_factories::use_installer(game, stack, pos, claimed);
}
