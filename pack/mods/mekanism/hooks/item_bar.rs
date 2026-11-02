// The bar under an item: a cube's energy.
fn mek_item_bar(stack: &horizon_client::items::ItemStack, bar: &mut Option<(i32, u32)>) {
    crate::game::mek_ui::item_bar(stack, bar);
}
