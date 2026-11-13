// `/ftbchunks` and its alias `/chunks`.
fn ftbchunks_register_commands(
    commands: &mut Vec<crate::game::native_server::NativeCommand>,
) {
    crate::game::ftbchunks_world::register(commands);
}
