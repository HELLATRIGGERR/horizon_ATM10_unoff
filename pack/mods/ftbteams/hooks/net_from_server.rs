// The teams the server sends this window's client.
fn ftbteams_net_from_server(channel: &str, data: &[u8]) {
    crate::game::ftbteams_world::from_server(channel, data);
}
