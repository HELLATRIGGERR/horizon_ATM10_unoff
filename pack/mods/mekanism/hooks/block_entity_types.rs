// Mekanism's block entity types, one per id its blocks' entities carry, as fork main's ids were (the compat hook:
// upstream declares types only through a trait mod's register()).
fn mek_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::mek_world::block_entity_types(types);
}
