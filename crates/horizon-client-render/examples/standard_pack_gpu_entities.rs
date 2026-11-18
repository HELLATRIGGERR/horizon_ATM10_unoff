#[cfg(not(test))]
#[path = "../fixtures/standard_pack_gpu_entities.rs"]
mod fixture;

#[cfg(not(test))]
fn main() {
        anchor_overlay_faces: &[],
        anchor_overlay_lines: &[],
        anchor_overlay_occluded: &[],
    fixture::main();
}
