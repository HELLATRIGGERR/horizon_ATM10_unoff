// What the screen's hooks left for a frame with the game in hand.
fn ftbq_frame(game: &mut crate::game::Game<'_>) {
    crate::game::ftbq_ui::frame(game);
}
