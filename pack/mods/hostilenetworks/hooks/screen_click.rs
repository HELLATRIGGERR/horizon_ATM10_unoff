// A press on a machine's screen: the menu button it is on.
fn hostilenetworks_screen_click(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    crate::game::hostilenetworks_ui::click(game, block, data, at, right, button);
}
