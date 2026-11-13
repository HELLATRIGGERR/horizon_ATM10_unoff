// A player's `/ftbteams` command on the server.
fn ftbteams_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::ftbteams_world::perform(game, player, command, ran);
}
