// The `/hnn` command.
fn hostilenetworks_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::hostilenetworks_world::commands(commands);
}
