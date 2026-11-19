// The window leaves its world or server: the region cache is written and dropped.
fn ftbchunks_client_disconnecting() {
    crate::game::ftbchunks_largemap::world_closed();
}
