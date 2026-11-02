// A message from a player's client.
fn ftbq_net_from_client(player: u128, channel: &str, data: &[u8]) {
    crate::game::ftbq_world::from_client(player, channel, data);
}
