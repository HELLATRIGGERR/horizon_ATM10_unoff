// The furnaces' block entity types, one per block and named after it, as fork main's ids were (the compat
// hook: upstream declares types only through a trait mod's register()).
fn ironfurnaces_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::ironfurnaces_world::block_entity_types(types);
}
