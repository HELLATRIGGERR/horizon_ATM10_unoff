// A right click on the infusion altar or a pedestal: an item goes on or comes off; the wand activates the altar.
// The click ends there, as Java's `SUCCESS`.
fn ma_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    crate::game::ma_world::right_click(game, click.pos, canceled);
    if *canceled {
        *result = mod_api::InteractionResult::Success;
    }
}
