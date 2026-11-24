// A player right clicks a block: the station opens its menu, and the click ends there.
fn woodworking_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    if crate::game::woodworking_world::right_click(game, click) {
        *canceled = true;
        *result = mod_api::InteractionResult::Success;
    }
}
