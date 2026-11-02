// A player's server tick: messages, detection, the save.
fn ftbq_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::ftbq_world::player_tick(game);
}
