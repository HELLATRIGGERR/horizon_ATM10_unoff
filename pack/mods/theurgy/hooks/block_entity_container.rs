// The item slots hoppers see on an apparatus.
fn theurgy_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::theurgy_world::container(block, container);
}
