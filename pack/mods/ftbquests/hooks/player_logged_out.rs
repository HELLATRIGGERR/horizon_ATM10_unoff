// A player left: their progress is saved and forgotten.
fn ftbq_player_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::ftbq_world::logged_out(game, player);
}
