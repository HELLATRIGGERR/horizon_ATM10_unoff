// An explosion: claimed blocks stay unless the team allows explosions.
fn ftbchunks_explosion_detonate(
    dimension: &str,
    source: Option<(u32, &str)>,
    blocks: &mut Vec<[i32; 3]>,
) {
    crate::game::ftbchunks_world::explosion(dimension, source, blocks);
}
