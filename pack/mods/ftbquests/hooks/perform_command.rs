// `/ftbquests ...` and `/ftbquests_probe`.
fn ftbquests_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::ftbquests_world::perform(game, player, command, ran);
}
