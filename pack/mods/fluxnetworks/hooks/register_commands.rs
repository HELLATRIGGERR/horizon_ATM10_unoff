// `/flux` in every player's command tree.
fn flux_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::flux_world::commands(commands);
}
