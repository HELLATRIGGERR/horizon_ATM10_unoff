// A right click with the book item opens the book; the hook passes one call (its stack is the API's), answered by
// `use_item_answer`.
fn ftbquests_use_item(game: &mut crate::game::Game<'_>, call: &mod_api::__private::UseItemCall<'_>) {
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(call.stack, game.items()) else { return };
    crate::game::ftbquests_world::use_item(game, &stack);
}
