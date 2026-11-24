// The world closes (or fails to open): the settings, the recipe books and the machines' unsaved state are dropped.
fn oritech_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::oritech_world::lifecycle(phase);
}
