// A player left: the others see them offline.
fn ftbteams_player_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::ftbteams_world::logged_out(game, player);
}
