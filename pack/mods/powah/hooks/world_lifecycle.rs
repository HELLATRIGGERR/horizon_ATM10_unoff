// A world opens or closes: Powah's settings are read or dropped.
fn powah_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::powah_world::lifecycle(phase);
}
