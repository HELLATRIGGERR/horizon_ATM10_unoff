// A player's `/mek` command on the server.
fn mek_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::mek_world::perform(game, source, command, ran);
}
