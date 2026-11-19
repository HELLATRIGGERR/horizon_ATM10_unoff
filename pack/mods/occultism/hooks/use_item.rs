// An item used in the air: a running ritual that asks for it hears it. The use goes on.
fn occultism_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    let _ = claimed;
    crate::game::occultism_world::use_item(game, stack);
}
