// The woodworking station's block entity type (compat hook for the external mod's registration).
fn woodworking_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::woodworking_world::block_entity_types(types);
}
