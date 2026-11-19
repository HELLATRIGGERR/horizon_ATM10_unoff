// A world closes (or fails to open): the claims are written and dropped; in the window's own world the region
// cache too.
fn ftbchunks_world_lifecycle(phase: mod_api::WorldPhase) {
    if matches!(phase, mod_api::WorldPhase::WorldCloseFinal) {
        crate::game::ftbchunks_largemap::world_closed();
    }
    crate::game::ftbchunks_world::lifecycle(phase);
}
