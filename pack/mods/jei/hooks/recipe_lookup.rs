// Another mod's screen asks the recipe viewer for a stack's recipes or uses, or what a key over it does.
fn jei_recipe_lookup(
    game: &mut crate::game::Game<'_>,
    stack: &mod_api::NativeStack,
    lookup: &mod_api::NativeRecipeLookup,
    handled: &mut bool,
) {
    crate::game::jei_world::recipe_lookup(game, stack, lookup, handled);
}
