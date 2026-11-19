// An entity inside a mod block: items in spirit fire are kept for the level's end of tick.
fn occultism_block_entity_inside(dimension: &str, entity: i32, kind: &str, state: horizon_world::registry::BlockStateId, position: [i32; 3]) {
    let _ = state;
    crate::game::occultism_world::inside(dimension, entity, kind, position);
}
