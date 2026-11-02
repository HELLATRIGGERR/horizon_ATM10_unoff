// `/ftbquests` in every player's command tree.
fn ftbq_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::ftbq_world::commands(commands);
}
