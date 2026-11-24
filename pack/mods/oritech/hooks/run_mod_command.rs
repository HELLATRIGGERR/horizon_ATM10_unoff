// A player's `/oritech` command on the server.
fn oritech_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::oritech_world::perform(game, source, command, ran);
}
