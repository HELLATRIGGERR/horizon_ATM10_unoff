// A player uses an item on a block: a tier installer turns a machine into a factory, a factory into the next tier;
// a configurator configures, empties, turns or takes up a Mekanism block.
fn mek_use_item_on(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    crate::game::mek_factories::use_installer(game, stack, pos, claimed);
    crate::game::mek_configurator::use_on(game, stack, pos, face, claimed);
}
