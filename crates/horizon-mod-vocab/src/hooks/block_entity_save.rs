//! The `block_entity_save` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// A block entity of a mod is written for saving (autosave, unload, close), when `id` is yours
    /// (`BlockEntity.saveAdditional`): change `nbt`, the copy that goes to disk. The world's own
    /// entity keeps its compound. A chunk is saved only when it changed:
    /// `native_block_entity_changed` says so for what's kept outside `nbt`.
    #[hook(
        side = "server",
        host = "read",
        contract = "collect",
        out = "nbt",
        neoforge = "none (vanilla BlockEntity.saveAdditional)"
    )]
    fn block_entity_save();
}
