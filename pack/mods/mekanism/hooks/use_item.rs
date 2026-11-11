// A player uses an item in the air: a Geiger Counter reads the radiation where they stand, a Dosimeter their dose.
fn mek_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::mek_radiation::use_item(game, stack, claimed);
}
