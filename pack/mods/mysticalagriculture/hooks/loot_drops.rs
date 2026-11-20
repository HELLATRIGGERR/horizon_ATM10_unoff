// A crop of MA's broken: its essence, its seeds and the fertilized essence (`MysticalCropBlock.getDrops`).
fn ma_loot_drops(
    game: &crate::game::Game<'_>,
    context: &mod_api::LootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::ma_world::loot_drops(game, context, drops);
}
