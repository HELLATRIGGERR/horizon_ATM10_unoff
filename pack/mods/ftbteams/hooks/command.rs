// A command typed in this window's chat: the client's probe.
fn ftbteams_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::ftbteams_world::client_command(game, command, ran);
}
