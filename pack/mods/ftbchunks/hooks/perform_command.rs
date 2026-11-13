// A player's `/ftbchunks` command on the server.
fn ftbchunks_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::ftbchunks_world::perform(game, player, command, ran);
}
