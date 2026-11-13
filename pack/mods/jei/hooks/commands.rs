// The client's own commands (the tests' probe).
fn jei_commands(commands: &mut Vec<&'static str>) {
    crate::game::jei_world::client_commands(commands);
}
