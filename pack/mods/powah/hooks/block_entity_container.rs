// A Furnator's item slots, and the menu that shows them.
fn powah_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::powah_world::container(block, container);
}
