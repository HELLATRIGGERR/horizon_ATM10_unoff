// A release while JEI's config screen is open, over JEI's recipe window, or its bookmark list, stays theirs.
fn jei_screen_mouse_button_released_pre(screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::jei_config::released(screen, canceled);
    crate::game::jei_window::released(screen, canceled);
    crate::game::jei_bookmarks::released(screen, canceled);
}
