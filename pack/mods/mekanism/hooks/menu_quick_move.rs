// A shift-click in a Mekanism block's menu: the player's stack into a QIO dashboard's frequency.
fn mek_menu_quick_move(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut horizon_client::items::ItemStack,
    handled: &mut bool,
) {
    crate::game::mek_qio::quick_move(game, pos, entity, container_slots, slot, stack, handled);
}
