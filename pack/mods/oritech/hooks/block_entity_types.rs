// The processing machines' block entity types, one per block and named after it (the compat hook: upstream declares
// types only through a trait mod's register()).
fn oritech_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::oritech_world::block_entity_types(types);
}
