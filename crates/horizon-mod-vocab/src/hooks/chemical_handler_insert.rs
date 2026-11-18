//! The `chemical_handler_insert` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// `IChemicalHandler.insertChemical` on the block at `pos`, as `chemical_handler_tanks`:
    /// `amount` (1 or more) of `chemical` offered through `face`. Set `inserted` (0 to `amount`)
    /// when the block is yours and `inserted` is none yet; with `simulate`, answer what you would
    /// take and change nothing.
    #[hook(
        side = "server",
        contract = "replace",
        out = "inserted",
        neoforge = "none (Mekanism's IChemicalHandler.insertChemical)"
    )]
    fn chemical_handler_insert(
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        chemical: &str,
        amount: i64,
        simulate: bool,
        inserted: &mut Option<i64>,
    );
}
