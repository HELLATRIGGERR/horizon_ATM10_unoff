// `/ftbchunks` and its alias `/chunks`.
fn ftbchunks_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::ftbchunks_world::register(commands);
}
