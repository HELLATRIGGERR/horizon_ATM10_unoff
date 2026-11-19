// A machine's item slots, and the menu that shows them.
fn hostilenetworks_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::hostilenetworks_world::container(block, container);
}
