// `/ftbquests open_book`, and the tests' probe.
fn ftbquests_register_commands(
    commands: &mut Vec<crate::game::native_server::NativeCommand>,
) {
    crate::game::ftbquests_world::register(commands);
}
