//! The `recipe_categories` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// A recipe viewer asks for the mods' own recipe categories
    /// (`native_collect_recipe_categories`): push yours, each with its id, title, icon, catalysts
    /// and recipes, the outputs with their chances (a fishing spot, a sieve, a machine). Asked
    /// again whenever a viewer wants them; call `native_recipe_categories_changed` when yours
    /// change (the server's data came), so viewers ask again. Client-side.
    #[hook(
        side = "client",
        contract = "collect",
        out = "categories",
        neoforge = "none (JEI's IRecipeCategory and IRecipeRegistration, registered through the viewer's own API)"
    )]
    fn recipe_categories();
}
