// What the item keeps when a player breaks a Furnator.
fn powah_block_entity_drop(
    block: &str,
    entity: &horizon_client::containers::ModBlockEntity,
    by_player: bool,
    components: &mut horizon_world::nbt::Compound,
) {
    crate::game::powah_world::drop_components(block, entity, by_player, components);
}
