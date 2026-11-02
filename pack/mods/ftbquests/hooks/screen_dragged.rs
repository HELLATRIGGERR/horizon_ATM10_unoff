// A drag on the book.
fn ftbq_screen_dragged(game: &crate::game::Game<'_>, button: i32, delta: [f32; 2]) {
    crate::game::ftbq_ui::dragged(game, button, delta);
}
