// A player's `/ftbteams` command on the server.
fn ftbteams_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::ftbteams_world::perform(game, source, command, ran);
}
