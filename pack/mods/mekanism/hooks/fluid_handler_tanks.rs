// The fluid tanks of a Mekanism block on one side (`IFluidHandler` readback).
fn mek_fluid_handler_tanks(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &horizon_client::containers::ModBlockEntity,
    tanks: &mut Option<Vec<crate::game::native_hooks::NativeFluidTankInfo>>,
) {
    crate::game::mek_world::fluid_tanks(dimension, pos, face, entity, tanks);
}
