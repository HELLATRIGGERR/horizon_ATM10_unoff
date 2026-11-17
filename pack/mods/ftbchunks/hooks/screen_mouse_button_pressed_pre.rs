// A drag starts on the claim map or the large map; a sidebar button on the inventory.
fn ftbchunks_screen_mouse_button_pressed_pre(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::ftbchunks_map::pressed(game, screen, button, modifiers, canceled);
    crate::game::ftbchunks_largemap::pressed(game, screen, button, canceled);
}
