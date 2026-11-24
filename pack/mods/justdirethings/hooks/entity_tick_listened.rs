// The fluid drop hears the item entities' ticks (`EntityTickEvent`).
fn justdirethings_entity_tick_listened(listened: &mut bool) {
    crate::game::justdirethings_time::tick_listened(listened);
}
