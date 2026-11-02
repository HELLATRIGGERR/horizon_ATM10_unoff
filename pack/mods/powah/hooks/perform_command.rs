// A player's `/powah` command on the server.
fn powah_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::powah_world::perform(game, player, command, ran);
}
