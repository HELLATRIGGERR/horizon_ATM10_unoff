// A world opens or closes: Mekanism's settings are read or dropped, and the radiation sources in memory with them.
fn mek_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    crate::game::mek_world::lifecycle(phase);
    crate::game::mek_radiation::lifecycle();
}
