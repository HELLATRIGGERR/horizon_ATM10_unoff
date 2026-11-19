// The client's own commands (the tests' probe).
fn ftbchunks_register_client_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::ftbchunks_map::client_commands(commands);
}
