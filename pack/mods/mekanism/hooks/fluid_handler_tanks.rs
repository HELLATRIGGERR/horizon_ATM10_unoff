// The fluid tanks of a Mekanism block on one side (`IFluidHandler` readback).
fn mek_fluid_handler_tanks(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mod_api::ModBlockEntity,
    tanks: &mut Option<Vec<mod_api::NativeFluidTankInfo>>,
) {
    crate::game::mek_world::fluid_tanks(dimension, pos, face, entity, tanks);
}
