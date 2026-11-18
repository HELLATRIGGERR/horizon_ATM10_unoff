//! The `recipe_lookup` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// A mod asked the recipe viewer to show `stack` (`native_show_recipes`): its recipes, its
    /// uses, or what a key over it does in the viewer (`NativeRecipeLookup`). A recipe viewer acts
    /// on it and sets `handled`, so no other viewer does. Heard at the start of the frame after the
    /// ask, once for each, in the order asked. Client-side.
    #[hook(
        side = "client",
        contract = "cancel",
        out = "handled",
        neoforge = "none (JEI's IRecipesGui.show and EMI's EmiApi.displayRecipes/displayUses, called by other mods through the viewer's own API)",
        cancellable = true
    )]
    fn recipe_lookup();
}
