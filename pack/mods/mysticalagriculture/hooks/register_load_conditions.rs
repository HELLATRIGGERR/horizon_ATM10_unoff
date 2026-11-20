// MA's recipe load conditions (crop_enabled, crop_has_material, augment_enabled) under its own namespace.
fn ma_register_load_conditions(conditions: &mut dyn mod_api::LoadConditions) {
    crate::game::ma_world::register_conditions(conditions);
}
