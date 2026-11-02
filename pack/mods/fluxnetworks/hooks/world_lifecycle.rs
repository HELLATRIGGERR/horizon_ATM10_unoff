// A world opens or closes: the networks and settings are read or dropped.
fn flux_world_lifecycle(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::flux_world::lifecycle(game, phase);
}
