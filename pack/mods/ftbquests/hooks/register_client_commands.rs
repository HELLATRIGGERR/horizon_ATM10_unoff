// The client's probe command (tests).
fn ftbquests_register_client_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::ftbquests_book::client_commands(commands);
}
