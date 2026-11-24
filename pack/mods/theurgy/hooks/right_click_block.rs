// A right click on an apparatus: its slots, its tank, the brazier's fuel; a click taken ends there as Success.
fn theurgy_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    let main_hand = matches!(click.hand, mod_api::Hand::Main);
    crate::game::theurgy_world::right_click(game, click.pos, main_hand, canceled);
    if *canceled {
        *result = mod_api::InteractionResult::Success;
    }
}
