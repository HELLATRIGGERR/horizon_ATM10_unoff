// JEI's mod name line, last in each item tooltip, as `jei-mod-id-format.ini` formats it.
fn jei_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::jei_world::tooltip(game, stack, lines);
}
