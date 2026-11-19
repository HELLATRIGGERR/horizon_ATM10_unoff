// The client's own commands (the tests' probe).
fn jei_register_client_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::jei_world::client_commands(commands);
}
