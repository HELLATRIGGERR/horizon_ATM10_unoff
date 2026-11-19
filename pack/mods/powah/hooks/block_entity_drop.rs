// What the item keeps when a player breaks a Furnator.
fn powah_block_entity_drop(
    block: &str,
    entity: &mod_api::ModBlockEntity,
    by_player: bool,
    components: &mut horizon_core::nbt::Compound,
) {
    crate::game::powah_world::drop_components(block, entity, by_player, components);
}
