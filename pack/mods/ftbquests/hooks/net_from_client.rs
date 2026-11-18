// The book's checkmark and claim buttons.
fn ftbquests_net_from_client(
    game: &mut crate::game::Game<'_>,
    player: u128,
    channel: &str,
    data: &[u8],
) {
    crate::game::ftbquests_world::from_client(game, player, channel, data);
}
