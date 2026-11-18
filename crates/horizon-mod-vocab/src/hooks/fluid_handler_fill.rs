//! The `fluid_handler_fill` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// `IFluidHandler.fill` on the block entity at `pos`, a mod's: `resource` (never empty) offered
    /// through `face`, as in `fluid_handler_tanks`. Set `filled` (0 to the resource's amount) when
    /// `entity.id` is yours and `filled` is none yet; with `simulate` (`FluidAction.SIMULATE`),
    /// answer what you would take and change nothing. The entity is out of the world during the
    /// call, as in `fe_receive`; the engine puts it back and, when it took something for real,
    /// marks it changed (`setChanged`). The same block, side and operation asked again inside its
    /// own call gets 0, and so does a chain 16 calls deep.
    #[hook(
        side = "server",
        contract = "replace",
        out = "filled",
        neoforge = "Capabilities.FluidHandler.BLOCK (IFluidHandler.fill)"
    )]
    fn fluid_handler_fill(
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        entity: &mut ModBlockEntity,
        resource: &FluidStack,
        simulate: bool,
        filled: &mut Option<i32>,
    );
}
