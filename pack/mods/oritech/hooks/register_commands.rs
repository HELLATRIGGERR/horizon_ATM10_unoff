// The `/oritech` command.
fn oritech_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::oritech_world::commands(commands);
}
