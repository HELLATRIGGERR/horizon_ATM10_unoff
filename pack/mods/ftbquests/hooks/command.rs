// `/ftbquests` from the server's console.
fn ftbq_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::ftbq_world::console(game, command, ran);
}
