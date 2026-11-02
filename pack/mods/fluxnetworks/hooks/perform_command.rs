// A player's `/flux` command on the server.
fn flux_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::flux_world::perform(game, player, command, ran);
}
