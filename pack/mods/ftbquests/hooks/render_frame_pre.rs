// Each frame: the book opens when asked; the clicks' messages go out.
fn ftbquests_render_frame_pre(game: &mut crate::game::Game<'_>) {
    crate::game::ftbquests_book::frame(game);
}
