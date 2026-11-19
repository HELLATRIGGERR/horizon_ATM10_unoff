// A right click on a block: the bowls' slots, a golden bowl's ritual, spirit fire lit over datura; a click taken
// ends there as Success.
fn occultism_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    crate::game::occultism_world::right_click(game, click.pos, click.hit.face, canceled);
    if *canceled {
        *result = mod_api::InteractionResult::Success;
    }
}
