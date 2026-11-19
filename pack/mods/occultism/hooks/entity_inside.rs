// An entity inside a block (the fork's block_entity_inside; the hook passes one call): items in spirit fire are kept
// for the level's end of tick.
fn occultism_entity_inside(call: &super::fire::EntityInsideCall<'_>) {
    let at = [call.pos.x, call.pos.y, call.pos.z];
    crate::game::occultism_world::inside(call.dimension, call.entity, call.kind, at);
}
