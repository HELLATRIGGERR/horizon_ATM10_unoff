// The tooltips of a Furnator's screen.
fn powah_screen_tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    crate::game::powah_ui::tooltip(block, data, at, lines);
}
