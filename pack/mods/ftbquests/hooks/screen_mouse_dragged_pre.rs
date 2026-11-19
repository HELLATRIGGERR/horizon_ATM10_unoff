// A drag pans the quest grid.
fn ftbquests_screen_mouse_dragged_pre(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    canceled: &mut bool,
) {
    crate::game::ftbquests_book::dragged(game, screen, canceled);
}
