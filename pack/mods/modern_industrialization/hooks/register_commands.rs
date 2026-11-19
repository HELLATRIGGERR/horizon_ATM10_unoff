// `/mi probe` and `/mi offer_fe`, for tests.
fn mi_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::mi_world::commands(commands);
}
