// A player's `/hnn` command on the server.
fn hostilenetworks_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::hostilenetworks_world::perform(game, player, command, ran);
}
