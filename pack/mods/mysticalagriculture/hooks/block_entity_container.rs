// The altar's and the pedestals' item slots.
fn ma_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::ma_world::container(block, container);
}
