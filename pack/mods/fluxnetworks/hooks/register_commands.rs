// `/flux` in every player's command tree.
fn flux_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::flux_world::commands(commands);
}
