// The `/powah` command.
fn powah_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::powah_world::commands(commands);
}
