//! The `fluid_handler_tanks` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// The fluid handler of the block entity at `pos` in `dimension`, a mod's, through `face`, as
    /// NeoForge's `IFluidHandler` reads it (`getTanks`, `getFluidInTank`, `getTankCapacity`).
    /// `face` is the side of this block the caller touches, as in `fe_storage`; none is the block
    /// itself, a null side. Set `tanks` when `entity.id` is yours and `tanks` is none yet; leave it
    /// none on a side without a handler (`getCapability` answering null), so pipes and buses see
    /// nothing there. Asked on the server, often, from inside other blocks' transfers and maybe
    /// while another level is in place: answer from `entity` and your own state, not from the
    /// world.
    #[hook(
        side = "server",
        host = "read",
        contract = "replace",
        out = "tanks",
        neoforge = "Capabilities.FluidHandler.BLOCK (IFluidHandler)"
    )]
    fn fluid_handler_tanks();
}
