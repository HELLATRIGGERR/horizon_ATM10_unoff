// A mod message from the server: the frequencies a QIO frequency select screen lists for this player.
fn mek_net_from_server(channel: &str, data: &[u8]) {
    crate::game::mek_qio::from_server(channel, data);
}
