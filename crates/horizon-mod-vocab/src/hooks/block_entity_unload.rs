//! The `block_entity_unload` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// A block entity of a mod is leaving the world. `removed`: its block was broken or replaced
    /// (`setRemoved`), told as `block_entity_load` is, with `entity` as it last was. Otherwise its
    /// chunk is unloading (`onChunkUnloaded`), told at once, with the chunk's last save snapshot
    /// taken already and `dimension` the level in place. Closing a world tells nothing:
    /// `native_world_lifecycle` does.
    #[hook(
        side = "server",
        contract = "listen",
        neoforge = "IBlockEntityExtension.onChunkUnloaded"
    )]
    fn block_entity_unload();
}
