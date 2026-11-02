// A click in a device's chest menu.
fn flux_menu_clicked(game: &mut crate::game::Game<'_>, uuid: u128, window: i32, slot: i32, mode: i32) {
    crate::game::flux_world::menu_clicked(game, uuid, window, slot, mode);
}
