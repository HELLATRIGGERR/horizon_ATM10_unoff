// The fluid tanks of an Oritech machine on one side (`IFluidHandler` readback).
fn oritech_fluid_handler_tanks(entity: &mod_api::ModBlockEntity, tanks: &mut Option<Vec<mod_api::NativeFluidTankInfo>>) {
    crate::game::oritech_world::fluid_tanks(entity, tanks);
}
