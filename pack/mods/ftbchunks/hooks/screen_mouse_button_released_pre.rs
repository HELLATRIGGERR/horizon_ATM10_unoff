// A drag ends on the claim map: one request.
fn ftbchunks_screen_mouse_button_released_pre(game: &crate::game::Game<'_>, button: i32, canceled: &mut bool) {
    crate::game::ftbchunks_map::released(game, button, canceled);
}
