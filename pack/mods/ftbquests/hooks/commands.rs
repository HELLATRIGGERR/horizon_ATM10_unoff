// The client's probe command (tests).
fn ftbquests_commands(
    commands: &mut Vec<&'static str>,
) {
    crate::game::ftbquests_book::client_commands(commands);
}
