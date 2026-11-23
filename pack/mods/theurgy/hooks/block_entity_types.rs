// Theurgy's apparatus declare one block entity type each, named after their blocks (a mixin bundle has no register()).
fn theurgy_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    crate::game::theurgy_world::block_entity_types(types);
}
