// The world closes (or fails to open): Mekanism's settings are dropped, and the radiation sources in memory with them.
fn mek_world_lifecycle(phase: mod_api::WorldPhase) {
    crate::game::mek_world::lifecycle(phase);
    crate::game::mek_radiation::lifecycle();
}
