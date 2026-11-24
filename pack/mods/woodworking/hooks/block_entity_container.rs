// The woodworking station's item slots and the menu that shows them.
fn woodworking_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::woodworking_world::container(block, container);
}
