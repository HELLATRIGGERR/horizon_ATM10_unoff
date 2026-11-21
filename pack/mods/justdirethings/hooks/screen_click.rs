// A press on the generator's screen: the menu button it is on.
fn justdirethings_screen_click(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    crate::game::justdirethings_ui::click(game, block, data, at, right, button);
}
