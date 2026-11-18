// `/ftbquestsbook` (tests).
fn ftbquests_command(
    game: &mut crate::game::Game<'_>,
    command: &str,
    ran: &mut bool,
) {
    crate::game::ftbquests_book::client_command(game, command, ran);
}
