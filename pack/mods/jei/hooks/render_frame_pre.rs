// Each frame: the list follows the world and the mod's on/off; Ctrl+O flips the overlay.
fn jei_render_frame_pre(game: &mut crate::game::Game<'_>) {
    crate::game::jei_world::frame(game);
}
