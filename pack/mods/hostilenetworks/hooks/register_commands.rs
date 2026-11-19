// The `/hnn` command.
fn hostilenetworks_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::hostilenetworks_world::commands(commands);
}
