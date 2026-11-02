// A message from the server.
fn ftbq_net_from_server(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::ftbq_ui::from_server(game, channel, data);
}
