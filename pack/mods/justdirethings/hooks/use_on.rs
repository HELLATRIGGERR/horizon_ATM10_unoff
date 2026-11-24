// A player uses an item on a block (`GooBlock_Base.useItemOn`, which the block lets through to the item here): a dead
// goo fed its tier's revive item comes alive. The hook passes one call (its stack is the API's), answered by
// `use_on_answer`.
fn justdirethings_use_on(game: &mut crate::game::Game<'_>, call: &mod_api::__private::UseOnCall<'_>) {
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(call.stack, game.items()) else { return };
    crate::game::justdirethings_world::use_on(game, &stack, call.pos);
}
