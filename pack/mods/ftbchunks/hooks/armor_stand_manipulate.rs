// A player takes from or puts on an armor stand: INTERACT_ENTITY.
fn ftbchunks_armor_stand_manipulate(
    game: &mut crate::game::Game<'_>,
    cancel: &mut bool,
) {
    crate::game::ftbchunks_world::interact_entity(game, "minecraft:armor_stand", cancel);
}
