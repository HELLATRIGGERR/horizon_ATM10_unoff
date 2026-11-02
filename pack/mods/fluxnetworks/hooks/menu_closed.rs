// A device's chest menu closed.
fn flux_menu_closed(uuid: u128, window: i32) {
    crate::game::flux_world::menu_closed(uuid, window);
}
