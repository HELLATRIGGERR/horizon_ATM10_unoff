// A player uses an item on a block: a tier upgrade turns a furnace into the next tier, an augment goes into its slot.
fn ironfurnaces_use_item_on(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    let _ = face;
    crate::game::ironfurnaces_world::use_item_on(game, stack, pos, claimed);
}
