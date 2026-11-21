// `/mek` in every player's command tree.
fn mek_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::mek_world::commands(commands);
}
