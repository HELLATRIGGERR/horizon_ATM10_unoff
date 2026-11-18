// A world opens or closes: the furnaces' settings are read or dropped.
fn ironfurnaces_world_lifecycle(
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::ironfurnaces_world::lifecycle(phase);
}
