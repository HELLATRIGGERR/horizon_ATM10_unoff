// An MI machine's screen: its size and slots.
fn mi_screen(block: &str, screen: &mut Option<mod_api::Screen>) {
    crate::game::mi_ui::screen(block, screen);
}
