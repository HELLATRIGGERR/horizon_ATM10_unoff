// A player joined: their client gets the claims at the next tick.
fn ftbchunks_player_logged_in(player: u128) {
    crate::game::ftbchunks_world::logged_in(player);
}
