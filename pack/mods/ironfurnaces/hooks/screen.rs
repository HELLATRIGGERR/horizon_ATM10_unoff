// A furnace's screen: its size and slots.
fn ironfurnaces_screen(
    block: &str,
    screen: &mut Option<crate::game::native_hooks::NativeScreen>,
) {
    crate::game::ironfurnaces_ui::screen(block, screen);
}
