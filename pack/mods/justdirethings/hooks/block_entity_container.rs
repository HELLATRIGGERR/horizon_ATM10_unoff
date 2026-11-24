// The generator's fuel slot, and the menu that shows it.
fn justdirethings_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::justdirethings_world::container(block, container);
}
