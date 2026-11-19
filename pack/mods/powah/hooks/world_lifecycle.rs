// The world closes (or fails to open): Powah's settings are dropped.
fn powah_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::powah_world::lifecycle(phase);
}
