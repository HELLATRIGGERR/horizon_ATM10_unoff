// The energy a Furnator's item holds, in its tooltip.
fn powah_tooltip(
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::powah_ui::item_tooltip(stack, lines);
}
