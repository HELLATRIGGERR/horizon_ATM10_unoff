// A machine's screen: its size and slots.
fn hostilenetworks_screen(
    block: &str,
    screen: &mut Option<crate::game::native_hooks::NativeScreen>,
) {
    crate::game::hostilenetworks_ui::screen(block, screen);
}
