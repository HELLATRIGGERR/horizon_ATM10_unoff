// A fluid container used on the Paradox Machine fills from its tank or empties into it (`ParadoxMachine.useItemOn`).
fn justdirethings_use_item_on(
    game: &mut crate::game::Game<'_>,
    state: horizon_world::registry::BlockStateId,
    click: &mod_api::BlockClick<'_>,
    answer: &mut Option<mod_api::InteractionResult>,
) {
    crate::game::justdirethings_time::use_item_on(game, state, click, answer);
}
