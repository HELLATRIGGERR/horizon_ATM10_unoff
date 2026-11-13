// A command typed in this window's chat: the client's probe.
fn jei_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::jei_world::client_command(game, command, ran);
}
