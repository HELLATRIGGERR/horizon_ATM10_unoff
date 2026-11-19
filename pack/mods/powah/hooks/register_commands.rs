// The `/powah` command.
fn powah_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::powah_world::commands(commands);
}
