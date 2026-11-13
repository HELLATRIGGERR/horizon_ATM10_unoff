// A recipe viewer collects the mods' recipe categories: Mekanism's machines'.
fn mek_recipe_categories(game: &mut crate::game::Game<'_>, categories: &mut Vec<horizon_client::mod_api::NativeRecipeCategory>) {
    crate::game::mek_recipe_viewer::categories(game, categories);
}
