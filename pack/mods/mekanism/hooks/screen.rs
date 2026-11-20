// The layout of a Mekanism block's screen.
fn mek_screen(block: &str, screen: &mut Option<mod_api::Screen>) {
    crate::game::mek_ui::screen(block, screen);
}
