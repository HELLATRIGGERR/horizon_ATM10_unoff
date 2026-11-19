// What the item of a broken plug or point keeps.
fn flux_block_entity_drop(
    block: &str,
    entity: &mod_api::ModBlockEntity,
    components: &mut horizon_core::nbt::Compound,
) {
    crate::game::flux_world::drop_components(block, entity, components);
}
