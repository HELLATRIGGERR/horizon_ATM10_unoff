// `/ftbquests open_book`, and the tests' probe.
fn ftbquests_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::ftbquests_world::register(commands);
}
