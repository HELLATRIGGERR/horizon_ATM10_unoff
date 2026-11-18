// The world opens or closes: the teams' progress is read, or written and dropped.
fn ftbquests_world_lifecycle(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::ftbquests_world::lifecycle(game, phase);
}
