// `/ftbquests ...` and `/ftbquests_probe`.
fn ftbquests_run_mod_command(
    game: &mut crate::game::Game<'_>,
    source: &mod_api::CommandSource,
    command: &mod_api::CommandInvocation,
    ran: &mut bool,
) {
    crate::game::ftbquests_world::perform(game, source, command, ran);
}
