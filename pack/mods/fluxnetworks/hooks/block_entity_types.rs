// The devices' block entity types, one per device and named after its block, as fork main's ids were (the compat
// hook: upstream declares types only through a trait mod's register()).
fn flux_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::flux_world::block_entity_types(types);
}
