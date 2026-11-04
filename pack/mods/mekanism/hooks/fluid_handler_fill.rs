// A fluid offered to a Mekanism block (`IFluidHandler.fill`).
fn mek_fluid_handler_fill(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mut horizon_client::containers::ModBlockEntity,
    resource: &horizon_world::fluid_stack::FluidStack,
    simulate: bool,
    filled: &mut Option<i32>,
) {
    crate::game::mek_world::fluid_fill(dimension, pos, face, entity, resource, simulate, filled);
}
