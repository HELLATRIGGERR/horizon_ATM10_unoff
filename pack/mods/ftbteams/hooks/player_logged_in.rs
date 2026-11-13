// A player joined: their team, and the teams to their client.
fn ftbteams_player_logged_in(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    crate::game::ftbteams_world::logged_in(game, player, name);
}
