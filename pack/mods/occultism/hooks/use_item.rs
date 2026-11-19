// An item used in the air: a running ritual that asks for it hears it. The use goes on (the hook passes one call,
// its stack the API's).
fn occultism_use_item(game: &mut crate::game::Game<'_>, call: &mod_api::__private::UseItemCall<'_>) {
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(call.stack, game.items()) else { return };
    crate::game::occultism_world::use_item(game, &stack);
}
