// Each server tick: the team events, then the force-load tickets.
fn ftbchunks_server_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::ServerTickPhase,
) {
    if matches!(phase, crate::game::native_lifecycle::ServerTickPhase::ServerTickEnd { world_running: true }) {
        crate::game::ftbchunks_world::tick(game);
    }
}
