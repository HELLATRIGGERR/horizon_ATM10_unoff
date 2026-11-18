//! The `chemical_handler_extract` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// `IChemicalHandler.extractChemical(stack, action)` on the block at `pos`, as
    /// `chemical_handler_insert`: at most `amount` of `chemical` and only that one. Set `extracted`
    /// (0 to `amount`) to what leaves.
    #[hook(
        side = "server",
        contract = "replace",
        out = "extracted",
        neoforge = "none (Mekanism's IChemicalHandler.extractChemical)"
    )]
    fn chemical_handler_extract(
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        chemical: &str,
        amount: i64,
        simulate: bool,
        extracted: &mut Option<i64>,
    );
}
