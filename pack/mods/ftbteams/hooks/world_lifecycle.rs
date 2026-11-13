// A world opens or closes: the teams are read, or written and dropped.
fn ftbteams_world_lifecycle(game: &mut crate::game::Game<'_>, phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::ftbteams_world::lifecycle(game, phase);
}
