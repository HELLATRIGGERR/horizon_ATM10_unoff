// A player's `/flux` command on the server.
fn flux_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::flux_world::perform(game, source, command, ran);
}
