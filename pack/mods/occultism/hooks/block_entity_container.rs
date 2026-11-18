// A bowl's slot, and a golden bowl's taken ingredients.
fn occultism_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::occultism_world::container(block, container);
}
