// `/mi probe` and `/mi offer_fe`, for tests.
fn mi_register_commands(commands: &mut Vec<mod_api::Command>) {
    crate::game::mi_world::commands(commands);
}
