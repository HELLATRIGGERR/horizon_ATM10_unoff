// A player uses an item on a block (`ItemStack.useOn`, once the block let the click through): a tier upgrade turns
// a furnace into the next tier, an augment goes into its slot. The hook passes one call (its stack is the API's),
// answered by `use_on_answer`.
fn ironfurnaces_use_on(game: &mut crate::game::Game<'_>, call: &mod_api::__private::UseOnCall<'_>) {
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(call.stack, game.items()) else { return };
    crate::game::ironfurnaces_world::use_on(game, &stack, call.pos);
}
