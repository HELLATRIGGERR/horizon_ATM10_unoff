// The tooltips of a machine's screen.
fn oritech_screen_tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    crate::game::oritech_ui::tooltip(block, data, at, lines);
}
