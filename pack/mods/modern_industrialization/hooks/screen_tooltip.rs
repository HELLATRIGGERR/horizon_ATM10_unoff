// The tooltips of an MI machine's energy bar, efficiency bar and fluid slots.
fn mi_screen_tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    crate::game::mi_ui::tooltip(block, data, at, lines);
}
