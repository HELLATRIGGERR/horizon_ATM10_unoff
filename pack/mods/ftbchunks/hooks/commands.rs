// The client's own commands (the tests' probe).
fn ftbchunks_commands(commands: &mut Vec<&'static str>) {
    crate::game::ftbchunks_map::client_commands(commands);
}
