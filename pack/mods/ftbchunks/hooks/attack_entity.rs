// A player hits an entity: frames, paintings, boats and carts are guarded.
fn ftbchunks_attack_entity(
    game: &mut crate::game::Game<'_>,
    kind: &str,
    pos: [f64; 3],
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::attack_entity(game, kind, pos, canceled);
}
