//! The `block_entity_load` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// A block entity of a mod entered the world: placed, or loaded with its chunk
    /// (`BlockEntity.onLoad`). Told on the server at the start of its level's next running block
    /// entity pass, before any of them tick, oldest first, after `block_entity_unload` of those
    /// that left with their block since. Check `entity.id`; it is out of the world during the call,
    /// as in `native_block_entity_tick`, so an old save's data can be changed here, and `dimension`
    /// is the level in place. One that came and left with its chunk before then isn't told at all.
    #[hook(
        side = "server",
        contract = "listen",
        neoforge = "none (vanilla BlockEntity.onLoad, the fork's server pass)"
    )]
    fn block_entity_load();
}
