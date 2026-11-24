// The tank of the cauldron, the accumulator or the sal ammoniac tank, on every side.
fn theurgy_fluid_handler_tanks(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mod_api::ModBlockEntity,
    tanks: &mut Option<Vec<mod_api::NativeFluidTankInfo>>,
) {
    let _ = (dimension, pos, face);
    crate::game::theurgy_world::fluid_tanks(entity, tanks);
}
