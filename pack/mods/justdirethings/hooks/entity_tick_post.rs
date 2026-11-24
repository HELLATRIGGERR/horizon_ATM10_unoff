// An entity ticked (`EntityTickEvent.Post`): an item of a fluid drop's catalyst in a source of its fluid makes the
// recipe's fluid.
fn justdirethings_entity_tick_post(game: &mut crate::game::Game<'_>, entity: u32, kind: &str) {
    crate::game::justdirethings_time::entity_tick_post(game, entity, kind);
}
