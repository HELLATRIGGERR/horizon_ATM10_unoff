// The server opened its world: Mekanism's settings are read; no radiation sources are in memory yet.
fn mek_server_started() {
    crate::game::mek_world::opened();
    crate::game::mek_radiation::lifecycle();
}
