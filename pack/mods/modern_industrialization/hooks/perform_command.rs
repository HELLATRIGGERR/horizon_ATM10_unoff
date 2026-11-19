// Runs `/mi ...`.
fn mi_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::mi_world::perform(game, player, command, ran);
}
