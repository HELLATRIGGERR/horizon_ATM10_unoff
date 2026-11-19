// A player right-clicks a block: INTERACT_BLOCK; a refusal ends the click (`Fail`).
fn ftbchunks_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::right_click_block(game, click.pos, canceled);
    if *canceled {
        *result = mod_api::InteractionResult::Fail;
    }
}
