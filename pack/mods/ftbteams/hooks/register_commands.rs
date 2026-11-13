// `/ftbteams` in every player's command tree.
fn ftbteams_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::ftbteams_world::register(commands);
}
