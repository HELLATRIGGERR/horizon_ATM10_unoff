// The end of a server tick: every network's transfer cycle.
fn flux_server_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::ServerTickPhase,
) {
    crate::game::flux_world::server_tick(game, phase);
}
