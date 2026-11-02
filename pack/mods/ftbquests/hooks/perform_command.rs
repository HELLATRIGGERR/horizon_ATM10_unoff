// A player's `/ftbquests` on the server.
fn ftbq_perform_command(game: &mut crate::game::Game<'_>, player: &crate::game::native_server::NativeServerPlayer, command: &str, ran: &mut bool) {
    crate::game::ftbq_world::perform(game, player, command, ran);
}
