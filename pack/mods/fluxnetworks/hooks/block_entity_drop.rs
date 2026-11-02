// What the item of a broken plug or point keeps.
fn flux_block_entity_drop(
    block: &str,
    entity: &horizon_client::containers::ModBlockEntity,
    components: &mut horizon_world::nbt::Compound,
) {
    crate::game::flux_world::drop_components(block, entity, components);
}
