// A player right clicks a block: a Flux device opens its menu, and the click ends there.
fn flux_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    crate::game::flux_world::right_click(game, click.pos, canceled);
    if *canceled {
        *result = mod_api::InteractionResult::Success;
    }
}
