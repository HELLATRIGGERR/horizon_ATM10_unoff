// The wheel on JEI's recipe window: its pages, or its categories over the tabs; over the bookmark list its pages.
fn jei_screen_mouse_scrolled_pre(
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::jei_config::scrolled(screen, scroll, canceled);
    crate::game::jei_window::scrolled(screen, scroll, canceled);
    crate::game::jei_bookmarks::scrolled(screen, scroll, canceled);
}
