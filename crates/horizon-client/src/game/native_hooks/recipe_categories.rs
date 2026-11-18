use super::*;

impl Game<'_> {
    /// A recipe viewer asks for the mods' own recipe categories
    /// (`native_collect_recipe_categories`): push yours, each with its id, title, icon, catalysts
    /// and recipes, the outputs with their chances (a fishing spot, a sieve, a machine). Asked
    /// again whenever a viewer wants them; call `native_recipe_categories_changed` when yours
    /// change (the server's data came), so viewers ask again. Client-side.
    pub(in crate::game) fn native_recipe_categories(
        &mut self,
        categories: &mut Vec<mod_api::NativeRecipeCategory>,
    ) {
        self.fire_recipe_categories(categories);
    }
}
