// A machine's item slots, and the menu that shows them.
fn oritech_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::oritech_world::container(block, container);
}
