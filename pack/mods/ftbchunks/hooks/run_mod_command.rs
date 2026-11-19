// A player's `/ftbchunks` command on the server.
fn ftbchunks_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::ftbchunks_world::perform(game, source, command, ran);
}
