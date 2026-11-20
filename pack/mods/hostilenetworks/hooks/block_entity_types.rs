// The Simulation Chamber's and the Loot Fabricator's block entity types, one per block and named after it, as fork main's ids were (the compat
// hook: upstream declares types only through a trait mod's register()).
fn hostilenetworks_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::hostilenetworks_world::block_entity_types(types);
}
