// A press on the woodworking station's screen: the recipe it selects (or a scroll command).
fn woodworking_screen_click(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    crate::game::woodworking_ui::click(game, block, data, at, right, button);
}
