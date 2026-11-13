// A command typed in this window's chat: the client's probe.
fn ftbchunks_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::ftbchunks_map::client_command(game, command, ran);
}
