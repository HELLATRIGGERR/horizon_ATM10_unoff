//! The `block_entity_drop` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// Block `block`'s block entity is breaking, and drops the block's loot: put into `components`
    /// what the item it drops keeps (`copy_components` from a block entity in vanilla's block loot
    /// tables), as a component patch (`mymod:energy`). `by_player`: a player's tool breaks it
    /// (`Block.playerDestroy`), not an explosion, a command or a piston. A dropped stack of the
    /// block's own item takes them, unless the block is broken without drops. `native_place_block`
    /// hears the item placed again. Read `entity` alone: it runs from a level's own pass.
    #[hook(
        side = "server",
        contract = "collect",
        out = "components",
        neoforge = "none (vanilla copy_components in block loot tables)"
    )]
    fn block_entity_drop();
}
