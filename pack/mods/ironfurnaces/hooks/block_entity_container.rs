// A furnace's item slots, and the menu that shows them.
fn ironfurnaces_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::ironfurnaces_world::container(block, container);
}
