// The woodworking station's screen: its size and slots.
fn woodworking_screen(block: &str, screen: &mut Option<mod_api::Screen>) {
    crate::game::woodworking_ui::screen(block, screen);
}
