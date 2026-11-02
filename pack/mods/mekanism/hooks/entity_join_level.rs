// An entity joins a level: a broken cube's dropped item gets its energy.
fn mek_entity_join_level(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    entity: u32,
    kind: &str,
    pos: [f64; 3],
    loaded_from_disk: bool,
) {
    crate::game::mek_world::joined(game, dimension, entity, kind, pos, loaded_from_disk);
}
