// A fluid asked of a Mekanism block (`IFluidHandler.drain`).
fn mek_fluid_handler_drain(
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    entity: &mut horizon_client::containers::ModBlockEntity,
    resource: Option<&horizon_world::fluid_stack::FluidStack>,
    max: i32,
    simulate: bool,
    drained: &mut Option<horizon_world::fluid_stack::FluidStack>,
) {
    crate::game::mek_world::fluid_drain(dimension, pos, face, entity, resource, max, simulate, drained);
}
