// A player uses an item in the air: a Geiger Counter reads the radiation where they stand, a Dosimeter their dose. The
// hook passes one call (its stack is the API's), answered by `use_item_answer`.
fn mek_use_item(game: &mut crate::game::Game<'_>, call: &mod_api::__private::UseItemCall<'_>) {
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(call.stack, game.items()) else { return };
    let mut claimed = false;
    crate::game::mek_radiation::use_item(game, &stack, &mut claimed);
    crate::game::mek_radiation::set_use_claimed(claimed);
}
