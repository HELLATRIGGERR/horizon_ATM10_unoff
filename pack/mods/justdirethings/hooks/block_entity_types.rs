// The goo blocks' and the Generator T1's block entity types, one per block and named after it (the compat hook:
// upstream declares types only through a trait mod's register()).
fn justdirethings_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::justdirethings_world::block_entity_types(types);
}
