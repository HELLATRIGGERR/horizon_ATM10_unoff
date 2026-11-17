// A world opens or closes on the server thread: the bookmarks learn which folder is the open world's.
fn jei_world_lifecycle(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::jei_bookmarks::lifecycle(game, phase);
}
