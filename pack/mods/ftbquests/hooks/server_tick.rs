// Each server tick: the inventories every `detection_delay` ticks, the clients' progress.
fn ftbquests_server_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::ServerTickPhase,
) {
    if matches!(phase, crate::game::native_lifecycle::ServerTickPhase::ServerTickEnd { world_running: true }) {
        crate::game::ftbquests_world::tick(game);
    }
}
