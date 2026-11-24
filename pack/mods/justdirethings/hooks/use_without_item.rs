// A machine passes a click with the Ferricore Wrench or the Machine Settings Copier on to the item (`BaseMachineBlock.useWithoutItem`).
fn justdirethings_use_without_item(
    game: &mut crate::game::Game<'_>,
    state: horizon_world::registry::BlockStateId,
    answer: &mut Option<mod_api::InteractionResult>,
) {
    crate::game::justdirethings_world::use_without_item(game, state, answer);
}
