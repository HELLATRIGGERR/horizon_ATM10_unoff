// A mod message from a player's client: a QIO frequency typed on a screen.
fn mek_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::mek_qio::from_client(game, player, channel, data);
}
