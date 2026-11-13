// The claims the server sends this window's client.
fn ftbchunks_net_from_server(channel: &str, data: &[u8]) {
    crate::game::ftbchunks_map::from_server(channel, data);
}
