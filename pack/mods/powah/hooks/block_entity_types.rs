// The Furnators' block entity types, one per tier and named after its block, as fork main's ids were (the compat
// hook: upstream declares types only through a trait mod's register()).
fn powah_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::powah_world::block_entity_types(types);
}
