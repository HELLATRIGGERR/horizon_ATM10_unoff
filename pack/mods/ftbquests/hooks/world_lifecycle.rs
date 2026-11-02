// A world opened or closed: nothing of one stays for the next.
fn ftbq_world_lifecycle(game: &mut crate::game::Game<'_>, phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::ftbq_world::lifecycle(game, phase);
}
