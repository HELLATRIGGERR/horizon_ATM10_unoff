//! The `fluid_handler_drain` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// `IFluidHandler.drain` on the block entity at `pos`, a mod's, as `fluid_handler_fill`: with a
    /// `resource`, `drain(FluidStack)` (at most its amount, and only that fluid with those
    /// components), without one, `drain(int)` of at most `max` (1 or more) of whatever comes out
    /// first. Set `drained` to what leaves (the empty stack for nothing).
    #[hook(
        side = "server",
        contract = "replace",
        out = "drained",
        neoforge = "Capabilities.FluidHandler.BLOCK (IFluidHandler.drain)"
    )]
    fn fluid_handler_drain(
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        entity: &mut ModBlockEntity,
        resource: Option<&FluidStack>,
        max: i32,
        simulate: bool,
        drained: &mut Option<FluidStack>,
    );
}
