// An MI machine's item slots, and the menu that shows them.
fn mi_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::mi_world::container(block, container);
}
