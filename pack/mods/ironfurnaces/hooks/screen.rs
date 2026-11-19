// A furnace's screen: its size and slots.
fn ironfurnaces_screen(block: &str, screen: &mut Option<mod_api::Screen>) {
    crate::game::ironfurnaces_ui::screen(block, screen);
}
