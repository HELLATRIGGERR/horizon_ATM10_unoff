// A player's `/powah` command on the server.
fn powah_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::powah_world::perform(game, source, command, ran);
}
