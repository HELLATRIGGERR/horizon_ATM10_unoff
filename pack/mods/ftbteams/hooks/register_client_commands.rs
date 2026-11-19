// The client's own commands (the tests' probe).
fn ftbteams_register_client_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::ftbteams_world::client_commands(commands);
}
