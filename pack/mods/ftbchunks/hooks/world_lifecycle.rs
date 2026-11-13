// A world opens or closes: the claims are read, or written and dropped.
fn ftbchunks_world_lifecycle(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::ftbchunks_world::lifecycle(game, phase);
}
