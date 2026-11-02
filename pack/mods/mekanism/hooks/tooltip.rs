// An item's tooltip: a cube's stored energy and capacity.
fn mek_tooltip(stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    crate::game::mek_ui::item_tooltip(stack, lines);
}
