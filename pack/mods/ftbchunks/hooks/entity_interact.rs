// A player right-clicks an entity: INTERACT_ENTITY; a refusal ends the interaction (`Fail`).
fn ftbchunks_entity_interact(
    game: &mut crate::game::Game<'_>,
    kind: &str,
    result: &mut Option<mod_api::InteractionResult>,
) {
    let mut stop = false;
    crate::game::ftbchunks_world::interact_entity(game, kind, &mut stop);
    if stop {
        *result = Some(mod_api::InteractionResult::Fail);
    }
}
