// The world closes (or fails to open): the settings and the recipe book are dropped.
fn justdirethings_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::justdirethings_world::lifecycle(phase);
}
