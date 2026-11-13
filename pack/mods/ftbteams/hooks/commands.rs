// The client's own commands (the tests' probe).
fn ftbteams_commands(commands: &mut Vec<&'static str>) {
    crate::game::ftbteams_world::client_commands(commands);
}
