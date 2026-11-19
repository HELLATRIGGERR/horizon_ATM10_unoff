// `/ftbteams` in every player's command tree.
fn ftbteams_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::ftbteams_world::register(commands);
}
