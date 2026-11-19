// A player's `/hnn` command on the server.
fn hostilenetworks_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::hostilenetworks_world::perform(game, source, command, ran);
}
