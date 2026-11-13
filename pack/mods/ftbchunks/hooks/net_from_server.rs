// The claims the server sends this window's client, and its waypoints and deaths.
fn ftbchunks_net_from_server(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::ftbchunks_map::from_server(channel, data);
    crate::game::ftbchunks_minimap::from_server(game, channel, data);
}
