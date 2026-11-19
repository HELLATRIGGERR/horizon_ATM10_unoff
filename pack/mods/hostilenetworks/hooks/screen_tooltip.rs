// The tooltips of a machine's screen.
fn hostilenetworks_screen_tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    crate::game::hostilenetworks_ui::tooltip(block, data, at, lines);
}
