// A machine's screen: its size and slots.
fn oritech_screen(block: &str, screen: &mut Option<mod_api::Screen>) {
    crate::game::oritech_ui::screen(block, screen);
}
