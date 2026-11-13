// A player right-clicks an entity: INTERACT_ENTITY.
fn ftbchunks_interact_entity(
    game: &mut crate::game::Game<'_>,
    kind: &str,
    claimed: &mut bool,
) {
    crate::game::ftbchunks_world::interact_entity(game, kind, claimed);
}
