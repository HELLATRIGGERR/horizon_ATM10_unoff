// A player right-clicks with an item: RIGHT_CLICK_ITEM. The hook passes one call (its stack is the API's): what is
// refused is decided here, where the game can tell the player, and answered by `use_item_answer`.
fn ftbchunks_use_item(game: &mut crate::game::Game<'_>, call: &mod_api::__private::UseItemCall<'_>) {
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(call.stack, game.items()) else { return };
    crate::game::ftbchunks_world::use_item(game, &stack);
}
