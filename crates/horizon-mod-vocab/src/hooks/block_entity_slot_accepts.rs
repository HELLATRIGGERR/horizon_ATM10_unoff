//! The `block_entity_slot_accepts` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// Slot `slot` of block `block`'s container is offered `stack`, by a hopper or a click in its
    /// menu (`Container.canPlaceItem`, `Slot.mayPlace`): clear `accepts` when the block is yours
    /// and refuses it. `fuel`: the stack burns in a furnace. It only narrows what the container's
    /// `inputs` let through, and asks neither for what a mod's own tick does nor for the stacks
    /// already in a slot. Asked often, from any thread's menu or hopper: answer from the arguments
    /// alone.
    #[hook(
        side = "server",
        contract = "collect",
        out = "accepts",
        neoforge = "none (vanilla Container.canPlaceItem and Slot.mayPlace)"
    )]
    fn block_entity_slot_accepts();
}
