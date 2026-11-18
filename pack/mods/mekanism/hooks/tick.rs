// The client's tick: QIO frequencies typed on a screen go to the server.
fn mek_tick(game: &mut crate::game::Game<'_>) {
    crate::game::mek_qio::client_tick(game);
}
