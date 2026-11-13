// A client's message: the claim screen's request.
fn ftbchunks_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::ftbchunks_world::from_client(game, player, channel, data);
}
