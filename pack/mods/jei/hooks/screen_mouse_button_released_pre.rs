// A release over JEI's recipe window stays the window's.
fn jei_screen_mouse_button_released_pre(screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::jei_window::released(screen, canceled);
}
