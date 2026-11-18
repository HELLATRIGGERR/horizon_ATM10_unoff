// A player joined: their client gets their team's progress at the next tick.
fn ftbquests_player_logged_in(
    player: u128,
) {
    crate::game::ftbquests_world::logged_in(player);
}
